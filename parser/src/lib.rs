//! Parse SQL migration scripts and generate models from it.
//!
//! Usually this will be used from build script.
pub use self::error::*;

use self::column::Column;
use self::migration::Migration;
use self::model::{Field, Model};
use self::ty::Type;
use self::writer::Writer;
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
mod writer;

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
    out: impl Write,
) -> Result<(), std::io::Error> {
    // Generate preamble.
    let mut w = Writer::new(out);

    w.line("use porm::migration::Migration;")?;
    w.line("use std::borrow::Cow;")?;
    w.line("use std::time::SystemTime;")?;
    w.line("use tokio_postgres::{Error, GenericClient};")?;

    // Write models.
    for (table, model) in cx.models {
        let name = AsUpperCamelCase(&table);

        w.blank_line()?;
        w.begin(format_args!(r#"pub struct {}"#, name))?;

        if model.has_lifetime {
            w.end("<'a> {")?;
        } else {
            w.end(" {")?;
        }

        w.increase_indent();

        for (c, f) in &model.fields {
            w.begin(format_args!(r#"pub {c}: "#))?;

            if f.nullable {
                w.end(format_args!(r#"Option<{}>,"#, f.ty.for_field()))?;
            } else {
                w.end(format_args!(r#"{},"#, f.ty.for_field()))?;
            }
        }

        w.decrease_indent();
        w.line(r#"}"#)?;
        w.blank_line()?;

        if model.has_lifetime {
            w.line(format_args!("impl<'a> {name}<'a> {{"))?;
        } else {
            w.line(format_args!("impl {name} {{"))?;
        }

        w.increase_indent();

        // Write insert method.
        w.line("pub async fn insert<T: GenericClient>(&self, client: &T) -> Result<(), Error> {")?;

        w.increase_indent();
        w.begin(format_args!(r#"client.execute("INSERT INTO {table} ("#))?;

        for (i, c) in model.fields.keys().enumerate() {
            if i != 0 {
                w.append(", ")?;
            }

            w.append(format_args!("{c}"))?;
        }

        w.append(") VALUES (")?;

        for (i, n) in (1..=model.fields.len()).enumerate() {
            if i != 0 {
                w.append(", ")?;
            }

            w.append(format_args!("${n}"))?;
        }

        w.append(r#")", &["#)?;

        for (i, c) in model.fields.keys().enumerate() {
            if i != 0 {
                w.append(", ")?;
            }

            w.append(format_args!("&self.{c}"))?;
        }

        w.end(r#"]).await?;"#)?;

        w.line("Ok(())")?;
        w.decrease_indent();

        w.line("}")?;

        // Write select method.
        if !model.primary_key.is_empty() {
            w.blank_line()?;
            w.begin("pub async fn select<T: GenericClient>(client: &T")?;

            for c in &model.primary_key {
                let f = model.fields.get(c).unwrap();

                if f.nullable {
                    w.append(format_args!(", {}: Option<{}>", c, f.ty.for_param()))?;
                } else {
                    w.append(format_args!(", {}: {}", c, f.ty.for_param()))?;
                }
            }

            w.end(") -> Result<Option<Self>, Error> {")?;

            w.increase_indent();
            w.begin(format_args!(
                r#"let r = client.query_opt("SELECT * FROM {} WHERE "#,
                table
            ))?;

            for (i, c) in model.primary_key.iter().enumerate() {
                if i != 0 {
                    w.append("AND ")?;
                }

                w.append(format_args!("{} = ${}", c, i + 1))?;
            }

            w.append(r#"", &["#)?;

            for (i, c) in model.primary_key.iter().enumerate() {
                let f = model.fields.get(c).unwrap();

                if i != 0 {
                    w.append(", ")?;
                }

                if f.nullable || f.ty.pass_by_ref() {
                    w.append(format_args!("&{c}"))?;
                } else {
                    w.append(format_args!("{c}"))?;
                }
            }

            w.end("]).await?;")?;
            w.line("let r = match r {")?;

            w.increase_indent();
            w.line("Some(v) => v,")?;
            w.line("None => return Ok(None),")?;
            w.decrease_indent();

            w.line("};")?;
            w.blank_line()?;

            for (c, f) in &model.fields {
                if f.nullable {
                    w.line(format_args!(
                        r#"let {} = r.try_get::<_, Option<{}>>("{}")?;"#,
                        c,
                        f.ty.for_retrieve(),
                        c
                    ))?;
                } else {
                    w.line(format_args!(
                        r#"let {} = r.try_get::<_, {}>("{}")?;"#,
                        c,
                        f.ty.for_retrieve(),
                        c
                    ))?;
                }
            }

            w.blank_line()?;
            w.begin(r#"Ok(Some(Self { "#)?;

            for (i, (n, f)) in model.fields.iter().enumerate() {
                if i != 0 {
                    w.append(", ")?;
                }

                if f.ty.is_cow() {
                    if f.nullable {
                        w.append(format_args!("{n}: {n}.map(Cow::Owned)"))?;
                    } else {
                        w.append(format_args!("{n}: Cow::Owned({n})"))?;
                    }
                } else {
                    w.append(format_args!("{n}"))?;
                }
            }

            w.end(" }))")?;
            w.decrease_indent();

            w.line("}")?;
        }

        w.decrease_indent();
        w.line("}")?;
    }

    // Write migrations.
    w.blank_line()?;

    w.line(format_args!(
        r#"pub static MIGRATIONS: [Migration; {}] = ["#,
        migrations.len()
    ))?;

    w.increase_indent();

    for (name, script) in migrations {
        match name {
            Some(name) => {
                w.line("Migration {")?;

                w.increase_indent();
                w.line(format_args!("name: Some({}),", Literal::string(&name)))?;
                w.line(format_args!("script: {},", Literal::string(&script)))?;
                w.decrease_indent();

                w.line("},")?;
            }
            None => {
                w.line("Migration {")?;

                w.increase_indent();
                w.line("name: None,")?;
                w.line(format_args!("script: {},", Literal::string(&script)))?;
                w.decrease_indent();

                w.line("},")?;
            }
        }
    }

    w.decrease_indent();
    w.line("];")?;

    Ok(())
}

/// Context to parse migration scripts.
struct Context {
    models: FxHashMap<String, Model>,
}
