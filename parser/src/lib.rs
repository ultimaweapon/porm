//! Parse SQL migration scripts and generate models from it.
//!
//! Usually this will be used from build script.
use crate::migration::Migration;
use proc_macro2::Literal;
use std::fmt::{Debug, Display, Formatter};
use std::io::Write;

pub mod migration;

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
    // Generate preamble.
    writeln!(out, r#"use porm::migration::Migration;"#).map_err(ParseError::WriteCode)?;

    // Load migrations.
    let scripts = migrations.into_iter();
    let mut migrations = Vec::new();

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
            match stmt {
                _ => continue,
            }
        }

        migrations.push((name, script));
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
                Some(n) => write!(f, "couldn't read migration script {n}"),
                None => write!(f, "couldn't read migration script for version {v}"),
            },
            Self::ParseMigration(n, v, _) => match n {
                Some(n) => write!(f, "couldn't parse migration script {n}"),
                None => write!(f, "couldn't parse migration script for version {v}"),
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
            "CREATE TABLE foo (foo integer);CREATE TABLE bar (bar text);",
            "CREATE TABLE baz (\"baz\" timestamp with time zone);",
        ]);

        parse(&mut out, migrations).unwrap();

        // Check output.
        let out = String::from_utf8(out).unwrap();

        assert_eq!(
            out,
            r#"use porm::migration::Migration;

pub static MIGRATIONS: [Migration; 2] = [
    Migration {
        name: None,
        script: "CREATE TABLE foo (foo integer);CREATE TABLE bar (bar text);",
    },
    Migration {
        name: None,
        script: "CREATE TABLE baz (\"baz\" timestamp with time zone);",
    },
];
"#
        );
    }
}
