//! Tools module for LLM function calling
//!
//! Defines available tools and their execution logic.

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tool definition for OpenAI-compatible API
#[derive(Debug, Clone, Serialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

/// Function definition within a tool
#[derive(Debug, Clone, Serialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// A tool call from the LLM
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionCall,
}

/// Function call details
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Get the list of available tools
pub fn get_available_tools() -> Vec<Tool> {
    vec![Tool {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: "get_current_time".to_string(),
            description: "Get the current date and time in the user's local timezone".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    }]
}

/// Execute a tool call and return the result
pub fn execute_tool(tool_call: &ToolCall) -> String {
    match tool_call.function.name.as_str() {
        "get_current_time" => {
            let now = Local::now();
            serde_json::json!({
                "datetime": now.to_rfc3339(),
                "date": now.format("%Y-%m-%d").to_string(),
                "time": now.format("%H:%M:%S").to_string(),
                "timezone": now.format("%Z").to_string(),
                "day_of_week": now.format("%A").to_string(),
            })
            .to_string()
        }
        _ => serde_json::json!({
            "error": format!("Unknown tool: {}", tool_call.function.name)
        })
        .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_available_tools() {
        let tools = get_available_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function.name, "get_current_time");
        assert_eq!(tools[0].tool_type, "function");
    }

    #[test]
    fn test_execute_get_current_time() {
        let tool_call = ToolCall {
            id: "test_id".to_string(),
            tool_type: "function".to_string(),
            function: FunctionCall {
                name: "get_current_time".to_string(),
                arguments: "{}".to_string(),
            },
        };

        let result = execute_tool(&tool_call);

        // Parse the result as JSON to verify it contains expected fields
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("datetime").is_some());
        assert!(parsed.get("date").is_some());
        assert!(parsed.get("time").is_some());
        assert!(parsed.get("day_of_week").is_some());
    }

    #[test]
    fn test_execute_unknown_tool() {
        let tool_call = ToolCall {
            id: "test_id".to_string(),
            tool_type: "function".to_string(),
            function: FunctionCall {
                name: "unknown_tool".to_string(),
                arguments: "{}".to_string(),
            },
        };

        let result = execute_tool(&tool_call);
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("error").is_some());
    }

    #[test]
    fn test_tool_serialization() {
        let tools = get_available_tools();
        let json = serde_json::to_string(&tools).unwrap();
        assert!(json.contains("get_current_time"));
        assert!(json.contains("function"));
    }
}
