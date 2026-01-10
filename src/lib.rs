pub mod client;
pub mod models;
pub mod chat;
pub mod error;

pub use client::OpenRouterClient;
pub use models::{ChatMessage, Model, ModelInfo, ModelsResponse, Role, StreamDelta, StreamResponse};
pub use chat::ChatSession;
pub use error::{TuisterError, Result};
