pub use self::column::*;
pub use self::constraint::*;

use std::env::VarError;
use std::num::NonZero;
use std::path::PathBuf;
use thiserror::Error;

mod column;
mod constraint;

/// Reason why [crate::parse()] fails.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ParseError {
    /// Couldn't read directory.
    #[error("couldn't read directory")]
    ReadDirectory(#[source] std::io::Error),
    /// Couldn't get migration identifier from file path.
    #[error("couldn't get migration identifier from {0}")]
    GetMigrationId(PathBuf, #[source] Box<dyn std::error::Error>),
    /// Failed to get environment variable `OUT_DIR`.
    #[error("couldn't get output directory")]
    GetOutputDir(#[source] VarError),
    /// Couldn't read migration script.
    #[error("couldn't read script from migration {0}")]
    ReadMigration(String, #[source] Box<dyn std::error::Error>),
    /// Couldn't parse migration script.
    #[error("couldn't parse script from migration {0}")]
    ParseMigration(String, #[source] pg_query::Error),
    /// Migration contains unsupported table name.
    #[error("table name '{1}' on migration {0} is not supported")]
    UnsupportedTableName(String, String),
    /// Migration contains duplicated table.
    #[error("duplicated table '{1}' on migration {0}")]
    DuplicatedTable(String, String),
    /// Migration contains unknown table.
    #[error("unknown table '{1}' on migration {0}")]
    UnknownTable(String, String),
    /// Failed to parse column.
    #[error("couldn't parse column at line {1} on migration {0}")]
    Column(String, NonZero<u32>, #[source] ColumnError),
    /// Migration contains unknown column.
    #[error("unknown column '{2}' at line {1} from migration {0}")]
    UnknownColumn(String, NonZero<u32>, String),
    /// Failed to parse table constraint.
    #[error("couldn't parse table constraint at line {1} from migration {0}")]
    TableConstraint(String, NonZero<u32>, #[source] ConstraintError),
    /// Couldn't write generated code.
    #[error("couldn't write generated code")]
    WriteCode(#[source] std::io::Error),
}
