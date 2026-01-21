use crate::error::{Result, TuisterError};
use crate::models::{ChatMessage, ChatResponse, Model, ModelsResponse, StreamResponse};
use crate::tools::{execute_tool, get_available_tools, Tool, ToolCall};
use futures::StreamExt;
use reqwest::Client;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
const MAX_STREAM_BUFFER_BYTES: usize = 256 * 1024;
const MAX_TOOL_ARGS_BYTES: usize = 64 * 1024;
const MAX_TOOL_CALLS: usize = 16;
const MAX_TOOL_CALL_DEPTH: usize = 4;

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Tool>>,
}

#[derive(Clone)]
pub struct OpenRouterClient {
    client: Client,
    api_key: String,
}

impl OpenRouterClient {
    pub fn new(api_key: String) -> Result<Self> {
        let client = Client::builder()
            .use_rustls_tls()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self { client, api_key })
    }

    pub async fn fetch_models(&self) -> Result<Vec<Model>> {
        let response = self.client.get(OPENROUTER_MODELS_URL).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(TuisterError::ApiError(format!(
                "Failed to fetch models with status {}: {}",
                status, error_text
            )));
        }

        let models_response: ModelsResponse = response.json().await?;

        let mut models: Vec<Model> = models_response
            .data
            .into_iter()
            .map(|model_info| Model::new(model_info.id, model_info.name))
            .collect();

        // Sort models alphabetically by name
        models.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        Ok(models)
    }

    pub async fn send_message_streaming(
        &self,
        model_id: &str,
        messages: &[ChatMessage],
        tx: mpsc::UnboundedSender<String>,
    ) -> Result<()> {
        self.send_message_streaming_with_tools(
            model_id,
            messages.to_vec(),
            tx,
            0,
            MAX_TOOL_CALLS,
        )
            .await
    }

    /// Internal streaming implementation that handles tool calls recursively
    async fn send_message_streaming_with_tools(
        &self,
        model_id: &str,
        mut messages: Vec<ChatMessage>,
        tx: mpsc::UnboundedSender<String>,
        depth: usize,
        tool_calls_remaining: usize,
    ) -> Result<()> {
        if depth >= MAX_TOOL_CALL_DEPTH {
            return Err(TuisterError::ApiError(
                "Tool call depth limit exceeded".to_string(),
            ));
        }

        let request = ChatRequest {
            model: model_id.to_string(),
            messages: messages.clone(),
            stream: Some(true),
            tools: Some(get_available_tools()),
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

        // Accumulate tool call data by index
        let mut tool_call_ids: HashMap<usize, String> = HashMap::new();
        let mut tool_call_names: HashMap<usize, String> = HashMap::new();
        let mut tool_call_args: HashMap<usize, String> = HashMap::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);
            if buffer.len() > MAX_STREAM_BUFFER_BYTES {
                return Err(TuisterError::ApiError(
                    "Streaming buffer exceeded limit".to_string(),
                ));
            }

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
                            // Handle regular content
                            if let Some(content) = &choice.delta.content {
                                if tx.send(content.clone()).is_err() {
                                    return Ok(());
                                }
                            }

                            // Handle tool calls (accumulate partial data)
                            if let Some(tool_calls) = &choice.delta.tool_calls {
                                for tc in tool_calls {
                                    let idx = tc.index;
                                    if !tool_call_ids.contains_key(&idx)
                                        && tool_call_ids.len() >= MAX_TOOL_CALLS
                                    {
                                        return Err(TuisterError::ApiError(
                                            "Tool call limit exceeded".to_string(),
                                        ));
                                    }
                                    if let Some(id) = &tc.id {
                                        tool_call_ids.insert(idx, id.clone());
                                    }
                                    if let Some(func) = &tc.function {
                                        if let Some(name) = &func.name {
                                            tool_call_names.insert(idx, name.clone());
                                        }
                                        if let Some(args) = &func.arguments {
                                            let entry = tool_call_args.entry(idx).or_default();
                                            if entry.len() + args.len() > MAX_TOOL_ARGS_BYTES {
                                                return Err(TuisterError::ApiError(
                                                    "Tool arguments exceeded limit".to_string(),
                                                ));
                                            }
                                            entry.push_str(args);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // If we have accumulated tool calls, execute them
        if !tool_call_ids.is_empty() {
            let mut tool_calls: Vec<ToolCall> = Vec::new();

            for (idx, id) in &tool_call_ids {
                if let Some(name) = tool_call_names.get(idx) {
                    let arguments = tool_call_args.get(idx).cloned().unwrap_or_default();
                    tool_calls.push(ToolCall {
                        id: id.clone(),
                        tool_type: "function".to_string(),
                        function: crate::tools::FunctionCall {
                            name: name.clone(),
                            arguments,
                        },
                    });
                }
            }

            if !tool_calls.is_empty() {
                if tool_calls.len() > tool_calls_remaining {
                    return Err(TuisterError::ApiError(
                        "Tool call limit exceeded".to_string(),
                    ));
                }
                // Send info about tool usage to UI
                for tc in &tool_calls {
                    let _ = tx.send(format!("\n[Using tool: {}]\n", tc.function.name));
                }

                // Add assistant message with tool calls to conversation
                messages.push(ChatMessage::assistant_with_tool_calls(tool_calls.clone()));

                // Execute each tool and add results
                for tc in &tool_calls {
                    let result = execute_tool(tc);
                    let _ = tx.send(format!("[Tool result: {}]\n", result));
                    messages.push(ChatMessage::tool_result(tc.id.clone(), result));
                }

                // Continue the conversation with tool results
                return Box::pin(self.send_message_streaming_with_tools(
                    model_id,
                    messages,
                    tx,
                    depth + 1,
                    tool_calls_remaining - tool_calls.len(),
                ))
                .await;
            }
        }

        Ok(())
    }

    pub async fn send_message(&self, model_id: &str, messages: &[ChatMessage]) -> Result<String> {
        let request = ChatRequest {
            model: model_id.to_string(),
            messages: messages.to_vec(),
            stream: None,
            tools: Some(get_available_tools()),
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
            choice
                .message
                .content
                .clone()
                .ok_or_else(|| TuisterError::ApiError("No content in response".to_string()))
        } else {
            Err(TuisterError::ApiError("No response from API".to_string()))
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
