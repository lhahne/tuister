use crate::client::OpenRouterClient;
use crate::error::Result;
use crate::models::{ChatMessage, Model};

pub struct ChatSession {
    client: OpenRouterClient,
    models: Vec<Model>,
    messages: Vec<ChatMessage>,
}

impl ChatSession {
    pub fn new(client: OpenRouterClient, models: Vec<Model>) -> Self {
        Self {
            client,
            models,
            messages: Vec::new(),
        }
    }
    
    pub fn add_system_message(&mut self, content: String) {
        self.messages.push(ChatMessage::system(content));
    }
    
    pub fn add_user_message(&mut self, content: String) {
        self.messages.push(ChatMessage::user(content));
    }
    
    pub fn models(&self) -> &[Model] {
        &self.models
    }
    
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }
    
    pub async fn send_to_model(&mut self, model: &Model) -> Result<String> {
        let response = self.client.send_message(&model.id, &self.messages).await?;
        Ok(response)
    }
    
    pub async fn send_to_all_models(&mut self) -> Result<Vec<(String, String)>> {
        let mut responses = Vec::new();
        let models = self.models.clone();
        
        for model in &models {
            match self.send_to_model(model).await {
                Ok(response) => {
                    responses.push((model.name.clone(), response));
                }
                Err(e) => {
                    responses.push((model.name.clone(), format!("Error: {}", e)));
                }
            }
        }
        
        Ok(responses)
    }
}
