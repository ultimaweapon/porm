use thiserror::Error;

/// Reason why a constraint fails to parse.
#[derive(Debug, Error)]
pub enum ConstraintError {
    /// Unknown column in primary key.
    #[error("unknown primary key '{0}'")]
    UnknownPrimaryKey(String),

    /// Unknown referenced table.
    #[error("unknown table '{0}'")]
    UnknownTable(String),
}
