use crate::error::{Result, TuisterError};
use crate::models::{ChatMessage, ChatResponse, Model, ModelsResponse, StreamResponse};
use futures::StreamExt;
use reqwest::Client;
use serde::Serialize;
use tokio::sync::mpsc;

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Clone)]
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
    
    pub async fn send_message_streaming(
        &self,
        model_id: &str,
        messages: &[ChatMessage],
        tx: mpsc::UnboundedSender<String>,
    ) -> Result<()> {
        let request = ChatRequest {
            model: model_id.to_string(),
            messages: messages.to_vec(),
            stream: Some(true),
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
        
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);
            
            // Process complete lines
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();
                
                // Skip empty lines and comments
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }
                
                // Parse SSE data
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        break;
                    }
                    
                    // Parse JSON chunk
                    if let Ok(stream_response) = serde_json::from_str::<StreamResponse>(data) {
                        if let Some(choice) = stream_response.choices.first() {
                            if let Some(content) = &choice.delta.content {
                                // Send the chunk to the UI
                                if tx.send(content.clone()).is_err() {
                                    // Receiver dropped, stop streaming
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    pub async fn send_message(
        &self,
        model_id: &str,
        messages: &[ChatMessage],
    ) -> Result<String> {
        let request = ChatRequest {
            model: model_id.to_string(),
            messages: messages.to_vec(),
            stream: None,
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
