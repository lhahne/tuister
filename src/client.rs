use crate::error::{Result, TuisterError};
use crate::models::{ChatMessage, ChatResponse};
use reqwest::Client;
use serde::Serialize;

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

pub struct OpenRouterClient {
    client: Client,
    api_key: String,
}

impl OpenRouterClient {
    pub fn new(api_key: String) -> Result<Self> {
        let client = Client::new();
        Ok(Self { client, api_key })
    }
    
    pub async fn send_message(
        &self,
        model_id: &str,
        messages: &[ChatMessage],
    ) -> Result<String> {
        let request = ChatRequest {
            model: model_id.to_string(),
            messages: messages.to_vec(),
        };
        
        let response = self
            .client
            .post(OPENROUTER_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(TuisterError::ApiError(format!(
                "API request failed with status {}: {}",
                status, error_text
            )));
        }
        
        let chat_response: ChatResponse = response.json().await?;
        
        if let Some(choice) = chat_response.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(TuisterError::ApiError(
                "No response from API".to_string(),
            ))
        }
    }
}
