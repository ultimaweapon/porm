//! Parse SQL migration scripts and generate models from it.
//!
//! Usually this will be used from build script.
pub use self::error::*;

use self::column::Column;
use self::model::{Field, Model};
use self::ty::Type;
use crate::migration::Migration;
use heck::AsUpperCamelCase;
use indexmap::IndexMap;
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
    mut out: impl Write,
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

    // Generate preamble.
    writeln!(
        out,
        r#"use porm::migration::Migration;
use tokio_postgres::{{Error, GenericClient}};"#
    )
    .map_err(ParseError::WriteCode)?;

    // Write models.
    for (table, data) in cx.models {
        let name = AsUpperCamelCase(&table);

        writeln!(
            out,
            r#"
pub struct {} {{"#,
            name
        )
        .map_err(ParseError::WriteCode)?;

        for (name, data) in &data.fields {
            write!(out, r#"    pub {}: "#, name).map_err(ParseError::WriteCode)?;

            if data.nullable {
                writeln!(out, r#"Option<{}>,"#, data.ty).map_err(ParseError::WriteCode)?;
            } else {
                writeln!(out, r#"{},"#, data.ty).map_err(ParseError::WriteCode)?;
            }
        }

        writeln!(out, r#"}}"#).map_err(ParseError::WriteCode)?;
        writeln!(out, "\nimpl {} {{", name).map_err(ParseError::WriteCode)?;

        // Write insert method.
        write!(
            out,
            r#"    pub async fn insert<T: GenericClient>(&self, client: &T) -> Result<(), Error> {{
        client.execute("INSERT INTO {} ("#,
            table
        )
        .map_err(ParseError::WriteCode)?;

        for (i, name) in data.fields.keys().enumerate() {
            if i == 0 {
                write!(out, "{name}").map_err(ParseError::WriteCode)?;
            } else {
                write!(out, ", {name}").map_err(ParseError::WriteCode)?;
            }
        }

        write!(out, ") VALUES (").map_err(ParseError::WriteCode)?;

        for (i, n) in (1..=data.fields.len()).enumerate() {
            if i == 0 {
                write!(out, "${n}").map_err(ParseError::WriteCode)?;
            } else {
                write!(out, ", ${n}").map_err(ParseError::WriteCode)?;
            }
        }

        write!(out, r#")", &["#).map_err(ParseError::WriteCode)?;

        for (i, name) in data.fields.keys().enumerate() {
            if i == 0 {
                write!(out, "&self.{name}").map_err(ParseError::WriteCode)?;
            } else {
                write!(out, ", &self.{name}").map_err(ParseError::WriteCode)?;
            }
        }

        writeln!(out, r#"]).await?;"#).map_err(ParseError::WriteCode)?;
        writeln!(out, "        Ok(())").map_err(ParseError::WriteCode)?;
        writeln!(out, "    }}").map_err(ParseError::WriteCode)?;
        writeln!(out, "}}").map_err(ParseError::WriteCode)?;
    }

    // Write migrations.
    writeln!(
        out,
        r#"
pub static MIGRATIONS: [Migration; {}] = ["#,
        migrations.len()
    )
    .map_err(ParseError::WriteCode)?;

    for (name, script) in migrations {
        match name {
            Some(name) => writeln!(
                out,
                r#"    Migration {{
        name: Some({}),
        script: {},
    }},"#,
                Literal::string(&name),
                Literal::string(&script)
            )
            .map_err(ParseError::WriteCode)?,
            None => writeln!(
                out,
                r#"    Migration {{
        name: None,
        script: {},
    }},"#,
                Literal::string(&script)
            )
            .map_err(ParseError::WriteCode)?,
        }
    }

    writeln!(out, "];").map_err(ParseError::WriteCode)?;

    Ok(())
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
    let mut fields = IndexMap::new();

    for def in defs {
        let def = match def.node {
            Some(v) => v,
            None => continue,
        };

        #[allow(clippy::single_match)] // TODO: Remove this.
        match def {
            Node::ColumnDef(v) => {
                use indexmap::map::Entry;

                // Check column name.
                let c = parse_column_def(v)?;

                if c.name.chars().any(|c| c.is_uppercase()) {
                    return Some(Err(ParseError::UnsupportedColumnName(
                        mn.clone(),
                        mv,
                        table,
                        c.name,
                    )));
                }

                // Check if exists.
                let e = match fields.entry(c.name) {
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

    e.insert(Model { fields });

    Some(Ok(()))
}

fn parse_column_def(node: Box<ColumnDef>) -> Option<Column> {
    let name = node.colname;
    let ty = node.type_name?;
    let ty = parse_column_type(ty.names)?;
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

fn parse_column_type(nodes: Vec<pg_query::protobuf::Node>) -> Option<Type> {
    let mut nodes = nodes.into_iter();
    let name = match nodes.next()?.node? {
        Node::String(v) => v.sval,
        _ => return None,
    };

    match name.as_str() {
        "pg_catalog" => nodes.next().and_then(parse_system_type),
        "serial" => Some(Type::Serial),
        "text" => Some(Type::Text),
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

/// Context to parse migration scripts.
struct Context {
    models: FxHashMap<String, Model>,
}
