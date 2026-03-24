use crate::ty::Type;
use rustc_hash::FxHashMap;

/// Contains information for a model.
pub struct Model {
    pub fields: FxHashMap<String, Field>,
}

/// Contains information for a field in model.
pub struct Field {
    pub ty: Type,
    pub nullable: bool,
}
