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
