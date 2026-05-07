/// Contains information for an index in the model.
pub struct Index {
    pub columns: Vec<IndexMember>,
    pub unique: bool,
}

/// Contains data for a member in the index.
pub struct IndexMember {
    pub column: String,
}
