use crate::error::{Result, TuisterError};
use crate::models::{ChatMessage, ChatResponse, Model, ModelsResponse};
use reqwest::Client;
use serde::Serialize;

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";

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
    
    pub async fn fetch_models(&self) -> Result<Vec<Model>> {
        let response = self
            .client
            .get(OPENROUTER_MODELS_URL)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(TuisterError::ApiError(format!(
                "Failed to fetch models with status {}: {}",
                status, error_text
            )));
        }
        
        let models_response: ModelsResponse = response.json().await?;
        
        let models = models_response
            .data
            .into_iter()
            .map(|model_info| Model::new(model_info.id, model_info.name))
            .collect();
        
        Ok(models)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = OpenRouterClient::new("test_api_key".to_string());
        assert!(client.is_ok());
    }

    #[test]
    fn test_api_key_storage() {
        let api_key = "my_secret_key".to_string();
        let client = OpenRouterClient::new(api_key.clone()).unwrap();
        assert_eq!(client.api_key, api_key);
    }
}
