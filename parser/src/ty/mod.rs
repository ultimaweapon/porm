use std::fmt::{Display, Formatter};

/// Data type on PostgreSQL.
pub enum Type {
    Integer,
    Text,
    TimestampWithTz,
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Integer => f.write_str("i32"),
            Self::Text => f.write_str("String"),
            Self::TimestampWithTz => f.write_str("::std::time::SystemTime"),
        }
    }
}
