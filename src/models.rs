use crate::tools::ToolCall;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn new(role: Role, content: String) -> Self {
        Self {
            role,
            content: Some(content),
            tool_calls: None,
            tool_call_id: None,
        }
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

    /// Create a tool result message
    pub fn tool_result(tool_call_id: String, result: String) -> Self {
        Self {
            role: Role::Tool,
            content: Some(result),
            tool_calls: None,
            tool_call_id: Some(tool_call_id),
        }
    }

    /// Create an assistant message with tool calls
    pub fn assistant_with_tool_calls(tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: None,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    /// Get content as string, returning empty string if None
    pub fn content_str(&self) -> &str {
        self.content.as_deref().unwrap_or("")
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
    #[serde(default)]
    pub tool_calls: Option<Vec<StreamToolCall>>,
}

/// Tool call in streaming response (may be partial)
#[derive(Debug, Clone, Deserialize)]
pub struct StreamToolCall {
    pub index: usize,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type", default)]
    pub tool_type: Option<String>,
    #[serde(default)]
    pub function: Option<StreamFunctionCall>,
}

/// Function call in streaming response (may be partial)
#[derive(Debug, Clone, Deserialize)]
pub struct StreamFunctionCall {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_creation() {
        let msg = ChatMessage::user("Hello".to_string());
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content_str(), "Hello");

        let msg = ChatMessage::assistant("Hi there".to_string());
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content_str(), "Hi there");

        let msg = ChatMessage::system("System message".to_string());
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.content_str(), "System message");
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
