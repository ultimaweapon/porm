pub use self::fk::*;
pub use self::index::*;

use crate::ty::Type;
use crate::{ConstraintError, Context};
use indexmap::IndexMap;
use pg_query::NodeEnum;
use pg_query::protobuf::{ConstrType, Constraint};

mod fk;
mod index;

/// Contains information for a model.
pub struct Model {
    pub table: String,
    pub fields: IndexMap<String, Field>,
    pub primary_key: Vec<String>,
    pub indexes: Vec<Index>,
    pub refs: Vec<Reference>,
    pub has_lifetime: bool,
}

impl Model {
    pub fn new(table: String) -> Self {
        Self {
            table,
            fields: IndexMap::new(),
            primary_key: Vec::new(),
            indexes: Vec::new(),
            refs: Vec::new(),
            has_lifetime: false,
        }
    }

    pub fn parse_table_constraint(
        &mut self,
        cx: &mut Context,
        node: Box<Constraint>,
    ) -> Result<(), ConstraintError> {
        let ty = node.contype.try_into().unwrap();

        match ty {
            ConstrType::ConstrPrimary => self.parse_pk(node)?,
            ConstrType::ConstrForeign => cx.foreign_keys.push(ForeignKey {
                migration: cx.migration,
                table: self.table.clone(),
                node,
            }),
            _ => (),
        }

        Ok(())
    }

    fn parse_pk(&mut self, node: Box<Constraint>) -> Result<(), ConstraintError> {
        for c in node.keys.into_iter().map(|n| n.node.unwrap()) {
            let c = match c {
                NodeEnum::String(v) => v.sval,
                _ => unreachable!(),
            };

            if !self.fields.contains_key(&c) {
                return Err(ConstraintError::UnknownPrimaryKey(c));
            }

            self.primary_key.push(c);
        }

        Ok(())
    }
}

/// Contains information for a field in the model.
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
