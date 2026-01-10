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

// OpenRouter Models API response structures
#[derive(Debug, Deserialize)]
pub struct ModelsResponse {
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub context_length: Option<u32>,
    #[serde(default)]
    pub pricing: Option<ModelPricing>,
}

#[derive(Debug, Deserialize)]
pub struct ModelPricing {
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub completion: String,
}

// Streaming response structures
#[derive(Debug, Deserialize)]
pub struct StreamResponse {
    pub id: String,
    pub choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
pub struct StreamChoice {
    pub delta: StreamDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StreamDelta {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_creation() {
        let msg = ChatMessage::user("Hello".to_string());
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");
        
        let msg = ChatMessage::assistant("Hi there".to_string());
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content, "Hi there");
        
        let msg = ChatMessage::system("System message".to_string());
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.content, "System message");
    }

    #[test]
    fn test_model_creation() {
        let model = Model::new("gpt-4", "GPT-4");
        assert_eq!(model.id, "gpt-4");
        assert_eq!(model.name, "GPT-4");
    }

    #[test]
    fn test_role_serialization() {
        let user_role = Role::User;
        let json = serde_json::to_string(&user_role).unwrap();
        assert_eq!(json, "\"user\"");
        
        let assistant_role = Role::Assistant;
        let json = serde_json::to_string(&assistant_role).unwrap();
        assert_eq!(json, "\"assistant\"");
        
        let system_role = Role::System;
        let json = serde_json::to_string(&system_role).unwrap();
        assert_eq!(json, "\"system\"");
    }

    #[test]
    fn test_chat_message_serialization() {
        let msg = ChatMessage::user("Test message".to_string());
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"user\""));
        assert!(json.contains("Test message"));
    }
}
