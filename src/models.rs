use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn new(role: Role, content: String) -> Self {
        Self { role, content }
    }
    
    pub fn user(content: String) -> Self {
        Self::new(Role::User, content)
    }
    
    pub fn assistant(content: String) -> Self {
        Self::new(Role::Assistant, content)
    }
    
    pub fn system(content: String) -> Self {
        Self::new(Role::System, content)
    }
}

#[derive(Debug, Clone)]
pub struct Model {
    pub id: String,
    pub name: String,
}

impl Model {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ChatMessage,
}
