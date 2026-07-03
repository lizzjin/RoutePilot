use std::collections::HashSet;

use serde_json::{Map, Value};

use crate::{config::ToolUseConfig, error::AppError, types::AnthropicRequest};

pub fn validate_anthropic_tooling(request: &AnthropicRequest) -> Result<(), AppError> {
    let tool_names = if let Some(tools) = request.extra.get("tools") {
        Some(validate_tool_definitions(tools)?)
    } else {
        None
    };

    if let Some(tool_choice) = request.extra.get("tool_choice") {
        validate_tool_choice_shape(tool_choice, tool_names.as_ref())?;
    }

    for (index, message) in request.messages.iter().enumerate() {
        validate_message_tool_blocks(message, index)?;
    }
    validate_tool_turn_references(request, tool_names.as_ref())?;

    Ok(())
}

pub fn validate_anthropic_tool_capabilities(
    request: &AnthropicRequest,
    provider_id: &str,
    tool_use: &ToolUseConfig,
) -> Result<(), AppError> {
    if !request_uses_tools(request) {
        return Ok(());
    }

    if !tool_use.supported {
        return Err(AppError::InvalidRequest(format!(
            "provider `{provider_id}` does not support tool use"
        )));
    }

    if request.extra.contains_key("tool_choice") && !tool_use.tool_choice {
        return Err(AppError::InvalidRequest(format!(
            "provider `{provider_id}` does not support tool_choice"
        )));
    }

    if !tool_use.parallel_tool_calls
        && request
            .extra
            .get("tool_choice")
            .and_then(|value| value.get("disable_parallel_tool_use"))
            .and_then(Value::as_bool)
            == Some(false)
    {
        return Err(AppError::InvalidRequest(format!(
            "provider `{provider_id}` does not support parallel tool calls"
        )));
    }

    Ok(())
}

fn validate_tool_definitions(tools: &Value) -> Result<HashSet<String>, AppError> {
    let tools = tools
        .as_array()
        .ok_or_else(|| AppError::InvalidRequest("tools must be an array".to_owned()))?;
    let mut names = HashSet::new();

    for (index, tool) in tools.iter().enumerate() {
        let path = format!("tools[{index}]");
        let object = tool
            .as_object()
            .ok_or_else(|| AppError::InvalidRequest(format!("{path} must be an object")))?;

        let name = object
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::InvalidRequest(format!("{path}.name is required")))?;
        validate_tool_name(name, &format!("{path}.name"))?;
        if !names.insert(name.to_owned()) {
            return Err(AppError::InvalidRequest(format!(
                "{path}.name duplicates another tool"
            )));
        }

        if object
            .get("description")
            .is_some_and(|description| !description.is_string())
        {
            return Err(AppError::InvalidRequest(format!(
                "{path}.description must be a string"
            )));
        }

        if let Some(schema) = object.get("input_schema") {
            let schema = schema.as_object().ok_or_else(|| {
                AppError::InvalidRequest(format!("{path}.input_schema must be an object"))
            })?;
            if schema
                .get("type")
                .is_some_and(|value| value.as_str() != Some("object"))
            {
                return Err(AppError::InvalidRequest(format!(
                    "{path}.input_schema.type must be object"
                )));
            }
        }
    }

    Ok(names)
}

fn request_uses_tools(request: &AnthropicRequest) -> bool {
    request.extra.contains_key("tools")
        || request.extra.contains_key("tool_choice")
        || request.messages.iter().any(message_has_tool_block)
}

fn message_has_tool_block(message: &Value) -> bool {
    message
        .get("content")
        .and_then(Value::as_array)
        .is_some_and(|blocks| {
            blocks.iter().any(|block| {
                matches!(
                    block.get("type").and_then(Value::as_str),
                    Some("tool_use" | "tool_result")
                )
            })
        })
}

fn validate_tool_choice_shape(
    tool_choice: &Value,
    tool_names: Option<&HashSet<String>>,
) -> Result<(), AppError> {
    let object = tool_choice
        .as_object()
        .ok_or_else(|| AppError::InvalidRequest("tool_choice must be an object".to_owned()))?;
    let choice_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::InvalidRequest("tool_choice.type is required".to_owned()))?;

    if !matches!(choice_type, "auto" | "any" | "none" | "tool") {
        return Err(AppError::InvalidRequest(
            "tool_choice.type must be auto, any, none, or tool".to_owned(),
        ));
    }
    if choice_type == "tool" {
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::InvalidRequest("tool_choice.name is required".to_owned()))?;
        validate_tool_name(name, "tool_choice.name")?;
        validate_tool_name_is_defined(name, tool_names, "tool_choice.name")?;
    } else if let Some(name) = object.get("name") {
        let Some(name) = name.as_str() else {
            return Err(AppError::InvalidRequest(
                "tool_choice.name must be a string".to_owned(),
            ));
        };
        validate_tool_name(name, "tool_choice.name")?;
        validate_tool_name_is_defined(name, tool_names, "tool_choice.name")?;
    }
    if matches!(choice_type, "any" | "tool") && tool_names.is_none_or(HashSet::is_empty) {
        return Err(AppError::InvalidRequest(format!(
            "tool_choice.type={choice_type} requires at least one tool"
        )));
    }

    if object
        .get("disable_parallel_tool_use")
        .is_some_and(|value| !value.is_boolean())
    {
        return Err(AppError::InvalidRequest(
            "tool_choice.disable_parallel_tool_use must be a boolean".to_owned(),
        ));
    }

    Ok(())
}

fn validate_tool_name_is_defined(
    name: &str,
    tool_names: Option<&HashSet<String>>,
    path: &str,
) -> Result<(), AppError> {
    if let Some(tool_names) = tool_names
        && !tool_names.contains(name)
    {
        return Err(AppError::InvalidRequest(format!(
            "{path} `{name}` must match a defined tool"
        )));
    }

    Ok(())
}

fn validate_tool_turn_references(
    request: &AnthropicRequest,
    tool_names: Option<&HashSet<String>>,
) -> Result<(), AppError> {
    let mut seen_tool_use_ids = HashSet::new();
    let mut pending_tool_use_ids = HashSet::new();

    for (message_index, message) in request.messages.iter().enumerate() {
        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
        let Some(blocks) = message.get("content").and_then(Value::as_array) else {
            continue;
        };

        for (block_index, block) in blocks.iter().enumerate() {
            let path = format!("messages[{message_index}].content[{block_index}]");
            let Some(object) = block.as_object() else {
                continue;
            };

            match object.get("type").and_then(Value::as_str) {
                Some("tool_use") if role == "assistant" => {
                    let id = object.get("id").and_then(Value::as_str).unwrap_or("");
                    if !seen_tool_use_ids.insert(id.to_owned()) {
                        return Err(AppError::InvalidRequest(format!(
                            "{path}.id duplicates a previous tool_use id"
                        )));
                    }
                    pending_tool_use_ids.insert(id.to_owned());

                    let name = object.get("name").and_then(Value::as_str).unwrap_or("");
                    validate_tool_name_is_defined(name, tool_names, &format!("{path}.name"))?;
                }
                Some("tool_result") if role == "user" => {
                    let tool_use_id = object
                        .get("tool_use_id")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if !seen_tool_use_ids.contains(tool_use_id) {
                        return Err(AppError::InvalidRequest(format!(
                            "{path}.tool_use_id `{tool_use_id}` does not match a previous tool_use id"
                        )));
                    }
                    if !pending_tool_use_ids.remove(tool_use_id) {
                        return Err(AppError::InvalidRequest(format!(
                            "{path}.tool_use_id `{tool_use_id}` has already been answered"
                        )));
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn validate_message_tool_blocks(message: &Value, message_index: usize) -> Result<(), AppError> {
    let Some(object) = message.as_object() else {
        return Ok(());
    };
    let role = object.get("role").and_then(Value::as_str).unwrap_or("");
    let Some(blocks) = object.get("content").and_then(Value::as_array) else {
        return Ok(());
    };

    for (block_index, block) in blocks.iter().enumerate() {
        let path = format!("messages[{message_index}].content[{block_index}]");
        let object = block
            .as_object()
            .ok_or_else(|| AppError::InvalidRequest(format!("{path} must be an object")))?;
        let block_type = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::InvalidRequest(format!("{path}.type is required")))?;

        match block_type {
            "text" => {
                let Some(text) = object.get("text") else {
                    return Err(AppError::InvalidRequest(format!("{path}.text is required")));
                };
                if !text.is_string() {
                    return Err(AppError::InvalidRequest(format!(
                        "{path}.text must be a string"
                    )));
                }
            }
            "tool_use" => validate_tool_use_block(role, object, &path)?,
            "tool_result" => validate_tool_result_block(role, object, &path)?,
            _ => {}
        }
    }

    Ok(())
}

fn validate_tool_use_block(
    role: &str,
    block: &Map<String, Value>,
    path: &str,
) -> Result<(), AppError> {
    if role != "assistant" {
        return Err(AppError::InvalidRequest(format!(
            "{path} tool_use blocks are only valid in assistant messages"
        )));
    }

    let id = block
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::InvalidRequest(format!("{path}.id is required for tool_use")))?;
    if id.trim().is_empty() {
        return Err(AppError::InvalidRequest(format!(
            "{path}.id must not be empty for tool_use"
        )));
    }

    let name = block
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::InvalidRequest(format!("{path}.name is required")))?;
    validate_tool_name(name, &format!("{path}.name"))?;

    if !block.get("input").is_some_and(Value::is_object) {
        return Err(AppError::InvalidRequest(format!(
            "{path}.input must be an object"
        )));
    }

    Ok(())
}

fn validate_tool_result_block(
    role: &str,
    block: &Map<String, Value>,
    path: &str,
) -> Result<(), AppError> {
    if role != "user" {
        return Err(AppError::InvalidRequest(format!(
            "{path} tool_result blocks are only valid in user messages"
        )));
    }

    let tool_use_id = block
        .get("tool_use_id")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::InvalidRequest(format!("{path}.tool_use_id is required")))?;
    if tool_use_id.trim().is_empty() {
        return Err(AppError::InvalidRequest(format!(
            "{path}.tool_use_id must not be empty"
        )));
    }

    if let Some(content) = block.get("content") {
        if !content.is_string() && !content.is_array() {
            return Err(AppError::InvalidRequest(format!(
                "{path}.content must be a string or array"
            )));
        }

        if let Some(blocks) = content.as_array() {
            for (index, block) in blocks.iter().enumerate() {
                if !block.is_object() {
                    return Err(AppError::InvalidRequest(format!(
                        "{path}.content[{index}] must be an object"
                    )));
                }
            }
        }
    }

    Ok(())
}

fn validate_tool_name(name: &str, path: &str) -> Result<(), AppError> {
    let len = name.chars().count();
    if len == 0 || len > 64 {
        return Err(AppError::InvalidRequest(format!(
            "{path} must be 1-64 characters"
        )));
    }

    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
    {
        return Err(AppError::InvalidRequest(format!(
            "{path} may only contain letters, numbers, '_' and '-'"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn skips_capability_gate_when_request_does_not_use_tools() {
        let request: AnthropicRequest = serde_json::from_value(json!({
            "model": "deepseek-v4-flash",
            "max_tokens": 128,
            "messages": [{
                "role": "user",
                "content": "hello"
            }]
        }))
        .unwrap();
        let tool_use = ToolUseConfig {
            supported: false,
            ..ToolUseConfig::default()
        };

        validate_anthropic_tool_capabilities(&request, "no_tools", &tool_use).unwrap();
    }

    #[test]
    fn rejects_parallel_tool_calls_when_provider_disallows_them() {
        let request: AnthropicRequest = serde_json::from_value(json!({
            "model": "deepseek-v4-flash",
            "max_tokens": 128,
            "tools": [{
                "name": "read_file",
                "input_schema": { "type": "object" }
            }],
            "tool_choice": {
                "type": "auto",
                "disable_parallel_tool_use": false
            },
            "messages": [{
                "role": "user",
                "content": "hello"
            }]
        }))
        .unwrap();
        let tool_use = ToolUseConfig {
            supported: true,
            tool_choice: true,
            parallel_tool_calls: false,
            ..ToolUseConfig::default()
        };

        let err =
            validate_anthropic_tool_capabilities(&request, "single_tool", &tool_use).unwrap_err();

        assert!(
            err.to_string()
                .contains("does not support parallel tool calls")
        );
    }
}
