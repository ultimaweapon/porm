use thiserror::Error;

/// Reason why a column fails to parse.
#[derive(Debug, Error)]
pub enum ColumnError {
    /// Name of the column is not supported.
    #[error("column name '{0}' is not supported")]
    UnsupportedName(String),

    /// Duplicated column.
    #[error("duplicated column '{0}'")]
    Duplicated(String),
}
