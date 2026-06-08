//! Relationship support — has_many, belongs_to.

use sea_orm::Value;
use serde::de::DeserializeOwned;

pub trait RelatedModel: DeserializeOwned + Send + Sync + 'static {}
impl<T: DeserializeOwned + Send + Sync + 'static> RelatedModel for T {}

/// Builder for relationship queries.
pub struct RelationBuilder<T: RelatedModel> {
    pub(crate) foreign_table: String,
    pub(crate) foreign_key: String,
    pub(crate) local_value: Value,
    _marker: std::marker::PhantomData<T>,
}

impl<T: RelatedModel> RelationBuilder<T> {
    pub fn has_many(
        foreign_table: impl Into<String>,
        foreign_key: impl Into<String>,
        local_value: impl Into<Value>,
    ) -> Self {
        Self {
            foreign_table: foreign_table.into(),
            foreign_key: foreign_key.into(),
            local_value: local_value.into(),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn belongs_to(
        parent_table: impl Into<String>,
        foreign_key: impl Into<String>,
        foreign_value: impl Into<Value>,
    ) -> Self {
        Self {
            foreign_table: parent_table.into(),
            foreign_key: foreign_key.into(),
            local_value: foreign_value.into(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Produce a SELECT SQL string for this relationship.
    pub fn to_sql(&self) -> String {
        format!(
            "SELECT * FROM \"{}\" WHERE \"{}\" = {}",
            self.foreign_table,
            self.foreign_key,
            crate::query::quote_value(&self.local_value)
        )
    }
}

/// Extension trait for models to add relationship helpers.
pub trait HasRelations: DeserializeOwned {
    fn has_many<R: RelatedModel>(
        &self,
        foreign_table: &str,
        foreign_key: &str,
        local_id: Value,
    ) -> RelationBuilder<R> {
        RelationBuilder::has_many(foreign_table, foreign_key, local_id)
    }

    fn belongs_to<R: RelatedModel>(
        &self,
        parent_table: &str,
        foreign_key: &str,
        foreign_id: Value,
    ) -> RelationBuilder<R> {
        RelationBuilder::belongs_to(parent_table, foreign_key, foreign_id)
    }
}

impl<T: DeserializeOwned> HasRelations for T {}
