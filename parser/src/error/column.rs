use thiserror::Error;

/// Reason why a column fails to parse.
#[derive(Debug, Error)]
pub enum ColumnError {
    #[error("column name '{0}' is not supported")]
    UnsupportedName(String),

    #[error("duplicated column '{0}'")]
    Duplicated(String),
}
