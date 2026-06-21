use pg_query::protobuf::Constraint;

/// Contains data for a foreign key constraint.
pub struct ForeignKey {
    pub migration: usize,
    pub table: String,
    pub node: Box<Constraint>,
}

/// Contains data for a reference to the table.
pub struct Reference {
    pub table: String,
    pub columns: Vec<String>,
    pub target: Vec<String>,
    pub ty: Option<RefType>,
}

/// Type of [Reference].
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RefType {
    OneToOne,
    OneToMany,
}
