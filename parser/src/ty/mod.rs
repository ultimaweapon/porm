/// Data type on PostgreSQL.
pub enum Type {
    BigInt,
    Boolean,
    Integer,
    Serial,
    SmallInt,
    Text,
    TimestampWithTz,
}

impl Type {
    pub fn for_field(&self) -> &'static str {
        match self {
            Self::BigInt => "i64",
            Self::Boolean => "bool",
            Self::Integer | Self::Serial => "i32",
            Self::SmallInt => "i16",
            Self::Text => "Cow<'a, str>",
            Self::TimestampWithTz => "SystemTime",
        }
    }

    pub fn for_builder(&self) -> &'static str {
        match self {
            Self::BigInt => "i64",
            Self::Boolean => "bool",
            Self::Integer | Self::Serial => "i32",
            Self::SmallInt => "i16",
            Self::Text => "&'a str",
            Self::TimestampWithTz => "SystemTime",
        }
    }

    pub fn for_param(&self) -> &'static str {
        match self {
            Self::BigInt => "i64",
            Self::Boolean => "bool",
            Self::Integer | Self::Serial => "i32",
            Self::SmallInt => "i16",
            Self::Text => "&str",
            Self::TimestampWithTz => "&SystemTime",
        }
    }

    pub fn for_retrieve(&self) -> &'static str {
        match self {
            Self::BigInt => "i64",
            Self::Boolean => "bool",
            Self::Integer => "i32",
            Self::Serial => "i32",
            Self::SmallInt => "i16",
            Self::Text => "String",
            Self::TimestampWithTz => "SystemTime",
        }
    }

    pub fn pass_by_ref(&self) -> bool {
        match self {
            Self::BigInt => true,
            Self::Boolean => true,
            Self::Integer => true,
            Self::Serial => true,
            Self::SmallInt => true,
            Self::Text => true,
            Self::TimestampWithTz => false,
        }
    }

    pub fn is_cow(&self) -> bool {
        match self {
            Self::BigInt => false,
            Self::Boolean => false,
            Self::Integer => false,
            Self::Serial => false,
            Self::SmallInt => false,
            Self::Text => true,
            Self::TimestampWithTz => false,
        }
    }
}
