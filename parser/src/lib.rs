//! Parse SQL migration scripts and generate models from it.
//!
//! Usually this will be used from build script.
pub use self::error::*;

use self::column::Column;
use self::model::{Field, Model};
use self::ty::Type;
use crate::migration::Migration;
use heck::AsUpperCamelCase;
use pg_query::protobuf::node::Node;
use pg_query::protobuf::{ColumnDef, ConstrType, CreateStmt};
use proc_macro2::Literal;
use rustc_hash::FxHashMap;
use std::io::Write;

pub mod migration;

mod column;
mod error;
mod model;
#[cfg(test)]
mod tests;
mod ty;

/// Parse SQL migration scripts and generate models from it.
///
/// The order of items produced by `migrations` must be the same every time.
pub fn parse<M, E>(
    out: impl Write,
    migrations: impl IntoIterator<Item = Result<M, E>>,
) -> Result<(), ParseError<E, M>>
where
    M: Migration,
    E: std::error::Error,
{
    // Load migrations.
    let scripts = migrations.into_iter();
    let mut migrations = Vec::new();
    let mut cx = Context {
        models: FxHashMap::default(),
    };

    for (version, migration) in scripts.enumerate() {
        // Get migration.
        let migration = migration.map_err(|e| ParseError::GetMigration(version, e))?;
        let name = migration.name();
        let script = match migration.read() {
            Ok(v) => v,
            Err(e) => return Err(ParseError::ReadMigration(name, version, e)),
        };

        // Parse migration.
        let parsed = match pg_query::parse(&script) {
            Ok(v) => v,
            Err(e) => return Err(ParseError::ParseMigration(name, version, e)),
        };

        for stmt in parsed.protobuf.stmts {
            // Get statement.
            let node = match stmt.stmt.and_then(|v| v.node) {
                Some(v) => v,
                None => continue,
            };

            // Process.
            let r = match node {
                Node::CreateStmt(n) => parse_create_stmt::<E, M>(&mut cx, &name, version, n),
                _ => continue,
            };

            if let Some(Err(e)) = r {
                return Err(e);
            }
        }

        migrations.push((name, script));
    }

    generate(cx, migrations, out).map_err(ParseError::WriteCode)
}

fn parse_create_stmt<I, M: Migration>(
    cx: &mut Context,
    mn: &Option<String>,
    mv: usize,
    node: CreateStmt,
) -> Option<Result<(), ParseError<I, M>>> {
    use std::collections::hash_map::Entry;

    // Check table name.
    let table = node.relation.map(|v| v.relname)?;

    if table.chars().any(|c| c.is_uppercase()) {
        return Some(Err(ParseError::UnsupportedTableName(mn.clone(), mv, table)));
    }

    // Parse create statement.
    let defs = node.table_elts;
    let mut model = Model::default();

    for def in defs {
        let def = match def.node {
            Some(v) => v,
            None => continue,
        };

        match def {
            Node::ColumnDef(v) => {
                use indexmap::map::Entry;

                // Check column name.
                let c = parse_column_def(&mut model, v)?;

                if c.name.chars().any(|c| c.is_uppercase()) {
                    return Some(Err(ParseError::UnsupportedColumnName(
                        mn.clone(),
                        mv,
                        table,
                        c.name,
                    )));
                }

                // Check if exists.
                let e = match model.fields.entry(c.name) {
                    Entry::Occupied(e) => {
                        return Some(Err(ParseError::DuplicatedColumn(
                            mn.clone(),
                            mv,
                            table,
                            e.key().clone(),
                        )));
                    }
                    Entry::Vacant(e) => e,
                };

                e.insert(Field {
                    ty: c.ty,
                    nullable: c.nullable,
                });
            }
            Node::Constraint(v) => model.parse_table_constraint(v),
            _ => (),
        }
    }

    // Check if exists.
    let e = match cx.models.entry(table) {
        Entry::Occupied(e) => {
            return Some(Err(ParseError::DuplicatedTable(
                mn.clone(),
                mv,
                e.key().clone(),
            )));
        }
        Entry::Vacant(e) => e,
    };

    e.insert(model);

    Some(Ok(()))
}

fn parse_column_def(model: &mut Model, node: Box<ColumnDef>) -> Option<Column> {
    let name = node.colname;
    let ty = node.type_name?;
    let ty = parse_column_type(model, ty.names)?;
    let mut nullable = true;

    for c in node.constraints {
        if let Some(Node::Constraint(v)) = c.node {
            match v.contype.try_into().unwrap() {
                ConstrType::ConstrNull => nullable = true,
                ConstrType::ConstrNotnull => nullable = false,
                _ => (),
            }
        }
    }

    Some(Column { name, ty, nullable })
}

fn parse_column_type(model: &mut Model, nodes: Vec<pg_query::protobuf::Node>) -> Option<Type> {
    let mut nodes = nodes.into_iter();
    let name = match nodes.next()?.node? {
        Node::String(v) => v.sval,
        _ => return None,
    };

    match name.as_str() {
        "pg_catalog" => nodes.next().and_then(parse_system_type),
        "serial" => Some(Type::Serial),
        "text" => {
            model.has_lifetime = true;

            Some(Type::Text)
        }
        _ => None,
    }
}

fn parse_system_type(node: pg_query::protobuf::Node) -> Option<Type> {
    let name = match node.node? {
        Node::String(v) => v.sval,
        _ => return None,
    };

    match name.as_str() {
        "int4" => Some(Type::Integer),
        "int8" => Some(Type::BigInt),
        "timestamptz" => Some(Type::TimestampWithTz),
        _ => None,
    }
}

fn generate(
    cx: Context,
    migrations: Vec<(Option<String>, String)>,
    mut out: impl Write,
) -> Result<(), std::io::Error> {
    // Generate preamble.
    writeln!(out, "use porm::migration::Migration;")?;
    writeln!(out, "use std::borrow::Cow;")?;
    writeln!(out, "use std::time::SystemTime;")?;
    writeln!(out, "use tokio_postgres::{{Error, GenericClient}};")?;

    // Write models.
    for (table, model) in cx.models {
        let name = AsUpperCamelCase(&table);

        writeln!(out)?;
        write!(out, r#"pub struct {}"#, name)?;

        if model.has_lifetime {
            writeln!(out, "<'a> {{")?;
        } else {
            writeln!(out, " {{")?;
        }

        for (c, f) in &model.fields {
            write!(out, r#"    pub {}: "#, c)?;

            if f.nullable {
                writeln!(out, r#"Option<{}>,"#, f.ty.for_field())?;
            } else {
                writeln!(out, r#"{},"#, f.ty.for_field())?;
            }
        }

        writeln!(out, r#"}}"#)?;

        if model.has_lifetime {
            writeln!(out, "\nimpl<'a> {}<'a> {{", name)?;
        } else {
            writeln!(out, "\nimpl {} {{", name)?;
        }

        // Write insert method.
        writeln!(
            out,
            "    pub async fn insert<T: GenericClient>(&self, client: &T) -> Result<(), Error> {{",
        )?;
        write!(out, r#"        client.execute("INSERT INTO {table} ("#)?;

        for (i, c) in model.fields.keys().enumerate() {
            if i == 0 {
                write!(out, "{c}")?;
            } else {
                write!(out, ", {c}")?;
            }
        }

        write!(out, ") VALUES (")?;

        for (i, n) in (1..=model.fields.len()).enumerate() {
            if i == 0 {
                write!(out, "${n}")?;
            } else {
                write!(out, ", ${n}")?;
            }
        }

        write!(out, r#")", &["#)?;

        for (i, c) in model.fields.keys().enumerate() {
            if i == 0 {
                write!(out, "&self.{c}")?;
            } else {
                write!(out, ", &self.{c}")?;
            }
        }

        writeln!(out, r#"]).await?;"#)?;
        writeln!(out, "        Ok(())")?;
        writeln!(out, "    }}")?;

        // Write select method.
        if !model.primary_key.is_empty() {
            writeln!(out)?;
            write!(out, "    pub async fn select<T: GenericClient>(client: &T")?;

            for c in &model.primary_key {
                let f = model.fields.get(c).unwrap();

                if f.nullable {
                    write!(out, ", {}: Option<{}>", c, f.ty.for_param())?;
                } else {
                    write!(out, ", {}: {}", c, f.ty.for_param())?;
                }
            }

            writeln!(out, ") -> Result<Option<Self>, Error> {{")?;
            write!(
                out,
                r#"        let r = client.query_opt("SELECT * FROM {} WHERE "#,
                table
            )?;

            for (i, c) in model.primary_key.iter().enumerate() {
                if i != 0 {
                    write!(out, "AND ")?;
                }

                write!(out, "{} = ${}", c, i + 1)?;
            }

            write!(out, r#"", &["#)?;

            for (i, c) in model.primary_key.iter().enumerate() {
                let f = model.fields.get(c).unwrap();

                if i != 0 {
                    write!(out, ", ")?;
                }

                if f.nullable || f.ty.pass_by_ref() {
                    write!(out, "&{c}")?;
                } else {
                    write!(out, "{c}")?;
                }
            }

            writeln!(out, "]).await?;")?;
            writeln!(out, "        let r = match r {{")?;
            writeln!(out, "            Some(v) => v,")?;
            writeln!(out, "            None => return Ok(None),")?;
            writeln!(out, "        }};")?;
            writeln!(out)?;

            for (c, f) in &model.fields {
                if f.nullable {
                    writeln!(
                        out,
                        r#"        let {} = r.try_get::<_, Option<{}>>("{}")?;"#,
                        c,
                        f.ty.for_retrieve(),
                        c
                    )?;
                } else {
                    writeln!(
                        out,
                        r#"        let {} = r.try_get::<_, {}>("{}")?;"#,
                        c,
                        f.ty.for_retrieve(),
                        c
                    )?;
                }
            }

            writeln!(out)?;
            write!(out, r#"        Ok(Some(Self {{ "#)?;

            for (i, (n, f)) in model.fields.iter().enumerate() {
                if i != 0 {
                    write!(out, ", ")?;
                }

                if f.ty.is_cow() {
                    if f.nullable {
                        write!(out, "{n}: {n}.map(Cow::Owned)")?;
                    } else {
                        write!(out, "{n}: Cow::Owned({n})")?;
                    }
                } else {
                    write!(out, "{n}")?;
                }
            }

            writeln!(out, " }}))")?;
            writeln!(out, "    }}")?;
        }

        writeln!(out, "}}")?;
    }

    // Write migrations.
    writeln!(out)?;
    writeln!(
        out,
        r#"pub static MIGRATIONS: [Migration; {}] = ["#,
        migrations.len()
    )?;

    for (name, script) in migrations {
        match name {
            Some(name) => {
                writeln!(out, "    Migration {{",)?;
                writeln!(out, "        name: Some({}),", Literal::string(&name))?;
                writeln!(out, "        script: {},", Literal::string(&script))?;
                writeln!(out, "    }},")?;
            }
            None => {
                writeln!(out, "    Migration {{",)?;
                writeln!(out, "        name: None,")?;
                writeln!(out, "        script: {},", Literal::string(&script))?;
                writeln!(out, "    }},")?;
            }
        }
    }

    writeln!(out, "];")?;

    Ok(())
}

/// Context to parse migration scripts.
struct Context {
    models: FxHashMap<String, Model>,
}
