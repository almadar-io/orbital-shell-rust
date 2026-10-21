//! What a host call can fail with.

use thiserror::Error;

/// Server-specific errors
#[derive(Debug, Error)]
pub enum HostError {
    /// Schema not found
    #[error("Schema not found: {0}")]
    SchemaNotFound(String),

    /// Orbital not found
    #[error("Orbital not found: {0}")]
    OrbitalNotFound(String),

    /// Trait not found
    #[error("Trait not found: {0}")]
    TraitNotFound(String),

    /// Entity not found
    #[error("Entity not found: {entity_type}:{id}")]
    EntityNotFound { entity_type: String, id: String },

    /// Invalid event
    #[error("Invalid event: {0}")]
    InvalidEvent(String),

    /// Guard evaluation failed
    #[error("Guard evaluation failed: {0}")]
    GuardEvaluation(String),

    /// Effect execution failed
    #[error("Effect execution failed: {0}")]
    EffectExecution(String),

    /// Persistence error
    #[error("Persistence error: {0}")]
    Persistence(String),

    /// Validation error
    #[error("Validation error: {0}")]
    Validation(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl HostError {
    /// Get HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            HostError::SchemaNotFound(_) => 404,
            HostError::OrbitalNotFound(_) => 404,
            HostError::TraitNotFound(_) => 404,
            HostError::EntityNotFound { .. } => 404,
            HostError::InvalidEvent(_) => 400,
            HostError::GuardEvaluation(_) => 422,
            HostError::EffectExecution(_) => 500,
            HostError::Persistence(_) => 500,
            HostError::Validation(_) => 400,
            HostError::Configuration(_) => 500,
            HostError::Internal(_) => 500,
            HostError::Json(_) => 400,
        }
    }
}
