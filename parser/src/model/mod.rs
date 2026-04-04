use crate::ConstraintError;
use crate::ty::Type;
use indexmap::IndexMap;
use pg_query::NodeEnum;
use pg_query::protobuf::{ConstrType, Constraint};

/// Contains information for a model.
#[derive(Default)]
pub struct Model {
    pub fields: IndexMap<String, Field>,
    pub primary_key: Vec<String>,
    pub has_lifetime: bool,
}

impl Model {
    pub fn parse_table_constraint(&mut self, node: Box<Constraint>) -> Result<(), ConstraintError> {
        let ty = node.contype.try_into().unwrap();

        #[allow(clippy::single_match)] // TODO: Remove this.
        match ty {
            ConstrType::ConstrPrimary => {
                for c in node.keys {
                    match c.node.unwrap() {
                        NodeEnum::String(v) => {
                            if !self.fields.contains_key(&v.sval) {
                                return Err(ConstraintError::UnknownPrimaryKey(v.sval));
                            }

                            self.primary_key.push(v.sval);
                        }
                        _ => continue,
                    };
                }
            }
            _ => (),
        }

        Ok(())
    }
}

/// Contains information for a field in model.
pub struct Field {
    pub ty: Type,
    pub nullable: bool,
    pub has_default: bool,
}

impl Field {
    pub fn is_optional(&self) -> bool {
        self.nullable || matches!(self.ty, Type::Serial) || self.has_default
    }
}
