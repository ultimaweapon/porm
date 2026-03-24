//! Parse SQL migration scripts and generate models from it.
//!
//! Usually this will be used from build script.
use self::column::Column;
use self::model::{Field, Model};
use self::ty::Type;
use crate::migration::Migration;
use heck::AsUpperCamelCase;
use pg_parse::ast::{ColumnDef, ConstrType, CreateStmt, Node};
use proc_macro2::Literal;
use rustc_hash::FxHashMap;
use std::fmt::{Debug, Display, Formatter};
use std::io::Write;

pub mod migration;

mod column;
mod model;
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
        let stmts = match pg_parse::parse(&script) {
            Ok(v) => v,
            Err(e) => return Err(ParseError::ParseMigration(name, version, e)),
        };

        for stmt in stmts {
            let r = match stmt {
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
    writeln!(out, r#"use porm::migration::Migration;"#).map_err(ParseError::WriteCode)?;

    // Write models.
    for (name, data) in cx.models {
        writeln!(
            out,
            r#"
pub struct {} {{"#,
            AsUpperCamelCase(name)
        )
        .map_err(ParseError::WriteCode)?;

        for (name, data) in data.fields {
            write!(out, r#"    pub {}: "#, name).map_err(ParseError::WriteCode)?;

            if data.nullable {
                writeln!(out, r#"Option<{}>,"#, data.ty).map_err(ParseError::WriteCode)?;
            } else {
                writeln!(out, r#"{},"#, data.ty).map_err(ParseError::WriteCode)?;
            }
        }

        writeln!(out, r#"}}"#).map_err(ParseError::WriteCode)?;
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
    let table = node.relation.and_then(|v| v.relname)?;

    if table.chars().any(|c| c.is_uppercase()) {
        return Some(Err(ParseError::UnsupportedTableName(mn.clone(), mv, table)));
    }

    // Parse create statement.
    let defs = node.table_elts?;
    let mut fields = FxHashMap::default();

    for def in defs {
        match def {
            Node::ColumnDef(v) => {
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
                    nullable: !c.is_not_null,
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

fn parse_column_def(node: ColumnDef) -> Option<Column> {
    let name = node.colname?;
    let ty = node.type_name?;
    let ty = ty.names.and_then(parse_column_type)?;
    let mut is_not_null = false;

    for c in node.constraints.unwrap_or_default() {
        match c {
            Node::Constraint(v) => match *v.contype {
                ConstrType::CONSTR_NULL => is_not_null = false,
                ConstrType::CONSTR_NOTNULL => is_not_null = true,
                _ => (),
            },
            _ => (),
        }
    }

    Some(Column {
        name,
        ty,
        is_not_null,
    })
}

fn parse_column_type(nodes: Vec<Node>) -> Option<Type> {
    let mut nodes = nodes.into_iter();
    let schema = match nodes.next()? {
        Node::String { sval } => sval?,
        _ => return None,
    };

    match schema.as_str() {
        "pg_catalog" => nodes.next().and_then(parse_system_type),
        "text" => Some(Type::Text),
        _ => None,
    }
}

fn parse_system_type(node: Node) -> Option<Type> {
    let name = match node {
        Node::String { sval } => sval?,
        _ => return None,
    };

    match name.as_str() {
        "int4" => Some(Type::Integer),
        "timestamptz" => Some(Type::TimestampWithTz),
        _ => None,
    }
}

/// Context to parse migration scripts.
struct Context {
    models: FxHashMap<String, Model>,
}

/// Reason why [parse()] fails.
pub enum ParseError<I, M>
where
    M: Migration,
{
    /// Couldn't get migration.
    GetMigration(usize, I),
    /// Couldn't read migration script.
    ReadMigration(Option<String>, usize, M::Err),
    /// Couldn't parse migration script.
    ParseMigration(Option<String>, usize, pg_parse::Error),
    /// Migration contains unsupported table name.
    UnsupportedTableName(Option<String>, usize, String),
    /// Migration contains duplicated table.
    DuplicatedTable(Option<String>, usize, String),
    /// Migration contains unsupported column name.
    UnsupportedColumnName(Option<String>, usize, String, String),
    /// Migration contains duplicated column.
    DuplicatedColumn(Option<String>, usize, String, String),
    /// Couldn't write generated code.
    WriteCode(std::io::Error),
}

impl<I, M> std::error::Error for ParseError<I, M>
where
    I: std::error::Error + 'static,
    M: Migration,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::GetMigration(_, e) => Some(e),
            Self::ReadMigration(_, _, e) => Some(e),
            Self::ParseMigration(_, _, e) => Some(e),
            Self::WriteCode(e) => Some(e),
            _ => None,
        }
    }
}

impl<I, M> Display for ParseError<I, M>
where
    M: Migration,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GetMigration(v, _) => write!(f, "couldn't get migration for version {v}"),
            Self::ReadMigration(n, v, _) => match n {
                Some(n) => write!(f, "couldn't read migration script '{n}'"),
                None => write!(f, "couldn't read migration script for version {v}"),
            },
            Self::ParseMigration(n, v, _) => match n {
                Some(n) => write!(f, "couldn't parse migration script '{n}'"),
                None => write!(f, "couldn't parse migration script for version {v}"),
            },
            Self::UnsupportedTableName(n, v, t) => match n {
                Some(n) => write!(f, "table name '{t}' on migration '{n}' is not supported"),
                None => write!(
                    f,
                    "table name '{t}' on migration version {v} is not supported"
                ),
            },
            Self::DuplicatedTable(n, v, t) => match n {
                Some(n) => write!(f, "duplicated table '{t}' on migration '{n}'"),
                None => write!(f, "duplicated table '{t}' on migration version '{v}'"),
            },
            Self::UnsupportedColumnName(n, v, t, c) => match n {
                Some(n) => write!(
                    f,
                    "unsupported column name '{c}' in table '{t}' on migration '{n}'"
                ),
                None => write!(
                    f,
                    "unsupported column name '{c}' in table '{t}' on migration version {v}"
                ),
            },
            Self::DuplicatedColumn(n, v, t, c) => match n {
                Some(n) => write!(
                    f,
                    "duplicated column '{c}' in table '{t}' on migration '{n}'"
                ),
                None => write!(
                    f,
                    "duplicated column '{c}' in table '{t}' on migration version {v}"
                ),
            },
            Self::WriteCode(_) => f.write_str("couldn't write generated code"),
        }
    }
}

impl<I, M> Debug for ParseError<I, M>
where
    I: Debug,
    M: Migration,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GetMigration(v, e) => f.debug_tuple("GetMigration").field(v).field(e).finish(),
            Self::ReadMigration(n, v, e) => f
                .debug_tuple("ReadMigration")
                .field(n)
                .field(v)
                .field(e)
                .finish(),
            Self::ParseMigration(n, v, e) => f
                .debug_tuple("ParseMigration")
                .field(n)
                .field(v)
                .field(e)
                .finish(),
            Self::UnsupportedTableName(n, v, t) => f
                .debug_tuple("UnsupportedTableName")
                .field(n)
                .field(v)
                .field(t)
                .finish(),
            Self::DuplicatedTable(n, v, t) => f
                .debug_tuple("DuplicatedTable")
                .field(n)
                .field(v)
                .field(t)
                .finish(),
            Self::UnsupportedColumnName(n, v, t, c) => f
                .debug_tuple("UnsupportedColumnName")
                .field(n)
                .field(v)
                .field(t)
                .field(c)
                .finish(),
            Self::DuplicatedColumn(n, v, t, c) => f
                .debug_tuple("DuplicatedColumn")
                .field(n)
                .field(v)
                .field(t)
                .field(c)
                .finish(),
            Self::WriteCode(e) => f.debug_tuple("WriteCode").field(e).finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::StrProvider;

    #[test]
    fn parse_with_valid() {
        // Parse.
        let mut out = Vec::new();
        let migrations = StrProvider::new([
            "CREATE TABLE foo (foo integer NOT NULL);CREATE TABLE bar (bar text);",
            "CREATE TABLE foo_bar (\"baz\" timestamp with time zone);",
        ]);

        parse(&mut out, migrations).unwrap();

        // Check output.
        let out = String::from_utf8(out).unwrap();

        assert_eq!(
            out,
            r#"use porm::migration::Migration;

pub struct Foo {
    pub foo: i32,
}

pub struct Bar {
    pub bar: Option<String>,
}

pub struct FooBar {
    pub baz: Option<::std::time::SystemTime>,
}

pub static MIGRATIONS: [Migration; 2] = [
    Migration {
        name: None,
        script: "CREATE TABLE foo (foo integer NOT NULL);CREATE TABLE bar (bar text);",
    },
    Migration {
        name: None,
        script: "CREATE TABLE foo_bar (\"baz\" timestamp with time zone);",
    },
];
"#
        );
    }
}
