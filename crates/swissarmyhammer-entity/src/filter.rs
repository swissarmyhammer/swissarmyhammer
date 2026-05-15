//! Predicate-based entity filtering with typed context.
//!
//! `EntityFilterContext` carries the full entity list plus caller-injected
//! typed extras, so predicates can make cross-entity decisions without the
//! entity layer knowing about domain types.

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::entity::Entity;

/// Context available to entity filter predicates.
///
/// Holds a reference to the full (unfiltered) entity list, an optional
/// "current" entity under evaluation, and an extensible bag of typed extras
/// that callers inject before filtering begins.
pub struct EntityFilterContext<'a> {
    /// The entity currently being evaluated (set per-iteration in virtual tag
    /// evaluation; `None` when the context is a shared template).
    pub entity: Option<&'a Entity>,
    /// All entities of the type being filtered (the unfiltered superset).
    pub entities: &'a [Entity],
    /// Typed extras — domain-specific state injected by the caller.
    extras: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl<'a> EntityFilterContext<'a> {
    /// Create a new filter context referencing the given entity slice.
    ///
    /// The `entity` field is `None`; use [`with_entity`](Self::with_entity)
    /// to set the current entity for per-item evaluation.
    pub fn new(entities: &'a [Entity]) -> Self {
        Self {
            entity: None,
            entities,
            extras: HashMap::new(),
        }
    }

    /// Create a filter context with a specific entity under evaluation.
    pub fn for_entity(entity: &'a Entity, entities: &'a [Entity]) -> Self {
        Self {
            entity: Some(entity),
            entities,
            extras: HashMap::new(),
        }
    }

    /// Insert a typed value into the extras bag.
    ///
    /// If a value of the same type was already present, it is replaced.
    pub fn insert<T: 'static + Send + Sync>(&mut self, value: T) {
        self.extras.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Retrieve a reference to a previously inserted typed value.
    ///
    /// Returns `None` if no value of type `T` has been inserted.
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.extras
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get_typed_values() {
        let entities = vec![];
        let mut ctx = EntityFilterContext::new(&entities);

        ctx.insert::<String>("hello".to_string());
        ctx.insert::<u64>(42);

        assert_eq!(ctx.get::<String>(), Some(&"hello".to_string()));
        assert_eq!(ctx.get::<u64>(), Some(&42));
    }

    #[test]
    fn get_missing_type_returns_none() {
        let entities = vec![];
        let ctx = EntityFilterContext::new(&entities);

        assert_eq!(ctx.get::<String>(), None);
        assert_eq!(ctx.get::<Vec<u8>>(), None);
    }

    #[test]
    fn insert_replaces_same_type() {
        let entities = vec![];
        let mut ctx = EntityFilterContext::new(&entities);

        ctx.insert::<String>("first".to_string());
        ctx.insert::<String>("second".to_string());

        assert_eq!(ctx.get::<String>(), Some(&"second".to_string()));
    }

    #[test]
    fn entities_field_is_accessible() {
        let entities = vec![Entity::new("task", "01A"), Entity::new("task", "01B")];
        let ctx = EntityFilterContext::new(&entities);

        assert_eq!(ctx.entities.len(), 2);
        assert_eq!(ctx.entities[0].id.as_ref(), "01A");
    }
}
