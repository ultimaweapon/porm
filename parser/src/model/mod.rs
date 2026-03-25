use crate::ty::Type;
use indexmap::IndexMap;

/// Contains information for a model.
pub struct Model {
    pub fields: IndexMap<String, Field>,
}

/// Contains information for a field in model.
pub struct Field {
    pub ty: Type,
    pub nullable: bool,
}
