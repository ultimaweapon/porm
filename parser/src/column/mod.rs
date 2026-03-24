use crate::ty::Type;

/// Contains column information.
pub struct Column {
    pub name: String,
    pub ty: Type,
    pub is_not_null: bool,
}
