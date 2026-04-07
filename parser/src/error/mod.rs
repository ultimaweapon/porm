pub use self::column::*;
pub use self::constraint::*;

use std::env::VarError;
use std::fmt::{Debug, Display, Formatter};
use std::num::NonZero;
use std::ops::Deref;
use std::path::PathBuf;

mod column;
mod constraint;

/// Reason why [crate::parse()] fails.
pub enum ParseError {
    /// Couldn't read directory.
    ReadDirectory(std::io::Error),
    /// Couldn't get migration identifier from file path.
    GetMigrationId(PathBuf, Box<dyn std::error::Error>),
    /// Failed to get environment variable `OUT_DIR`.
    GetOutputDir(VarError),
    /// Couldn't read migration script.
    ReadMigration(Option<String>, usize, Box<dyn std::error::Error>),
    /// Couldn't parse migration script.
    ParseMigration(Option<String>, usize, pg_query::Error),
    /// Migration contains unsupported table name.
    UnsupportedTableName(Option<String>, usize, String),
    /// Migration contains duplicated table.
    DuplicatedTable(Option<String>, usize, String),
    /// Migration contains unknown table.
    UnknownTable(Option<String>, usize, String),
    /// Failed to parse column.
    Column(Option<String>, usize, NonZero<u32>, ColumnError),
    /// Failed to parse table constraint.
    TableConstraint(Option<String>, usize, NonZero<u32>, ConstraintError),
    /// Couldn't write generated code.
    WriteCode(std::io::Error),
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadDirectory(e) => Some(e),
            Self::GetMigrationId(_, e) => Some(e.deref()),
            Self::GetOutputDir(e) => Some(e),
            Self::ReadMigration(_, _, e) => Some(e.deref()),
            Self::ParseMigration(_, _, e) => Some(e),
            Self::Column(_, _, _, e) => Some(e),
            Self::TableConstraint(_, _, _, e) => Some(e),
            Self::WriteCode(e) => Some(e),
            _ => None,
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadDirectory(_) => f.write_str("couldn't read directory"),
            Self::GetMigrationId(p, _) => {
                write!(f, "couldn't get migration identifier from {}", p.display())
            }
            Self::GetOutputDir(_) => f.write_str("couldn't get output directory"),
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
                None => write!(f, "duplicated table '{t}' on migration version {v}"),
            },
            Self::UnknownTable(n, v, t) => match n {
                Some(n) => write!(f, "unknown table '{t}' on migration '{n}'"),
                None => write!(f, "unknown table '{t}' on migration version {v}"),
            },
            Self::Column(n, v, l, _) => match n {
                Some(n) => write!(f, "couldn't parse column at line {l} on migration '{n}'"),
                None => write!(
                    f,
                    "couldn't parse column at line {l} on migration version {v}"
                ),
            },
            Self::TableConstraint(n, v, l, _) => match n {
                Some(n) => write!(
                    f,
                    "couldn't parse table constraint at line {l} from migration '{n}'"
                ),
                None => write!(
                    f,
                    "couldn't parse table constraint at line {l} from migration version {v}"
                ),
            },
            Self::WriteCode(_) => f.write_str("couldn't write generated code"),
        }
    }
}

impl Debug for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadDirectory(e) => f.debug_tuple("ReadDirectory").field(e).finish(),
            Self::GetMigrationId(p, e) => {
                f.debug_tuple("GetMigrationId").field(p).field(e).finish()
            }
            Self::GetOutputDir(e) => f.debug_tuple("GetOutputDir").field(e).finish(),
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
            Self::UnknownTable(n, v, t) => f
                .debug_tuple("UnknownTable")
                .field(n)
                .field(v)
                .field(t)
                .finish(),
            Self::Column(n, v, l, e) => f
                .debug_tuple("Column")
                .field(n)
                .field(v)
                .field(l)
                .field(e)
                .finish(),
            Self::TableConstraint(n, v, l, e) => f
                .debug_tuple("TableConstraint")
                .field(n)
                .field(v)
                .field(l)
                .field(e)
                .finish(),
            Self::WriteCode(e) => f.debug_tuple("WriteCode").field(e).finish(),
        }
    }
}
