//! SkillContext — wraps SkillLibrary for operation execution

use crate::skill_library::SkillLibrary;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Context for skill operations, providing access to the skill library
pub struct SkillContext {
    pub library: Arc<RwLock<SkillLibrary>>,
}

impl SkillContext {
    /// Create a new context wrapping a skill library
    pub fn new(library: Arc<RwLock<SkillLibrary>>) -> Self {
        Self { library }
    }
}
