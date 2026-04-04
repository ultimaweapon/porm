use thiserror::Error;

/// Reason why a constraint fails to parse.
#[derive(Debug, Error)]
pub enum ConstraintError {
    #[error("unknown primary key '{0}'")]
    UnknownPrimaryKey(String),
}
