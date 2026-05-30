use crate::{parse_jsonl_session, session_from_events, Event, Session, ToolCall};
use anyhow::{bail, Context};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

type TokenUsage = BTreeMap<String, i64>;

pub fn parse_file(path: &Path) -> anyhow::Result<Session> {
    if path.is_dir() {
        return parse_cline_task_dir(path);
    }
    if is_cline_task_file(path) {
        if let Some(dir) = path.parent() {
            if let Ok(session) = parse_cline_task_dir(dir) {
                return Ok(session);
            }
        }
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read session file {}", path.display()))?;
    let name = session_name(path);
    let path_text = path.to_string_lossy().to_string();
    parse_raw_session(&name, &path_text, &raw)
}

fn parse_cline_task_dir(dir: &Path) -> anyhow::Result<Session> {
    let metadata = read_json_value(&dir.join("task_metadata.json")).unwrap_or(Value::Null);
    let model = metadata
        .as_object()
        .map(cline_model)
        .unwrap_or_else(|| "unknown".to_string());
    let mut events = Vec::new();
    let mut seen = BTreeSet::new();

    if let Some(raw) = read_json_value(&dir.join("api_conversation_history.json")) {
        if let Some(api_events) = parse_cline_value(&raw, &model) {
            for event in api_events {
                append_cline_event(&mut events, &mut seen, event);
            }
        }
    }
    if let Some(raw) = read_json_value(&dir.join("ui_messages.json")) {
        for event in parse_cline_ui_messages(&raw, &model) {
            append_cline_event(&mut events, &mut seen, event);
        }
    }
    if events.is_empty() {
        bail!("cline: no parseable events in {}", dir.display());
    }
    if let Some(metadata) = metadata.as_object() {
        apply_cline_metadata_timestamps(&mut events, metadata);
    }
    let name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("cline-task")
        .to_string();
    session_from_events(&name, &dir.to_string_lossy(), events)
}

pub fn parse_raw_session(name: &str, path: &str, raw: &str) -> anyhow::Result<Session> {
    let trimmed = raw.trim();
    if is_aider_history(path, trimmed) {
        return session_from_events(name, path, parse_aider_chat_history(raw)?);
    }
    if trimmed.is_empty() {
        bail!("empty session");
    }
    let parsed_value = serde_json::from_str::<Value>(trimmed).ok();
    if let Some(value) = &parsed_value {
        if is_qwen_code_value(value) {
            return session_from_events(name, path, parse_qwen_code_value(value)?);
        }
        if let Some(events) = parse_openclaw_value(value) {
            return session_from_events(name, path, events);
        }
        if let Some(events) = parse_hermes_json_value(value) {
            return session_from_events(name, path, events);
        }
    }
    if parsed_value.is_none() {
        if let Some(events) = parse_codex_rollout_jsonl(raw) {
            return session_from_events(name, path, events);
        }
        if let Some(events) = parse_workbuddy_jsonl(raw) {
            return session_from_events(name, path, events);
        }
        if let Some(events) = parse_antigravity_jsonl(raw) {
            return session_from_events(name, path, events);
        }
        if let Some(events) = parse_cursor_transcript_jsonl(raw) {
            return session_from_events(name, path, events);
        }
        if let Some(events) = parse_claude_transcript_jsonl(raw) {
            return session_from_events(name, path, events);
        }
        if let Some(events) = parse_copilot_session_jsonl(raw) {
            return session_from_events(name, path, events);
        }
        if let Some(events) = parse_kimi_wire_jsonl(raw) {
            return session_from_events(name, path, events);
        }
    }
    if is_qwen_code_jsonl(raw) {
        return session_from_events(name, path, parse_qwen_code_jsonl(raw)?);
    }
    if is_oh_my_pi_jsonl(raw) {
        return session_from_events(name, path, parse_oh_my_pi_jsonl(path, raw)?);
    }
    if let Some(events) = parse_claude_code_jsonl(raw) {
        return session_from_events(name, path, events);
    }
    if let Some(events) = parse_copilot_jsonl(raw) {
        return session_from_events(name, path, events);
    }
    if let Ok(events) = serde_json::from_str::<Vec<Event>>(trimmed) {
        if !events.is_empty() {
            let mut events = events;
            for event in &mut events {
                if event.source_tool.is_empty() {
                    event.source_tool = "generic".to_string();
                }
            }
            return session_from_events(name, path, events);
        }
    }
    if let Some(value) = parsed_value {
        if let Some(events) = parse_opencode_storage_value(path, &value) {
            return session_from_events(name, path, events);
        }
        if let Some(events) = parse_cursor_export(&value) {
            return session_from_events(name, path, events);
        }
        if let Some(events) = parse_gemini_value(&value) {
            return session_from_events(name, path, events);
        }
        if let Some(events) = parse_kimi_value(&value) {
            return session_from_events(name, path, events);
        }
        if let Some(events) = parse_cline_value(&value, "unknown") {
            return session_from_events(name, path, events);
        }
        if let Some(events) = parse_messages_value(&value, "codex_cli") {
            return session_from_events(name, path, events);
        }
    }
    if let Ok(session) = parse_jsonl_session(name, path, raw) {
        return Ok(session);
    }
    bail!("unsupported session format: {}", path)
}

fn parse_copilot_session_jsonl(raw: &str) -> Option<Vec<Event>> {
    let entries = jsonl_objects(raw);
    if !entries
        .iter()
        .any(|entry| string(entry.get("type")) == Some("session.start"))
    {
        return None;
    }
    let mut events = Vec::new();
    for entry in entries {
        let typ = string(entry.get("type")).unwrap_or("");
        let timestamp = string(entry.get("timestamp")).unwrap_or("").to_string();
        let data = entry.get("data").and_then(Value::as_object);
        match typ {
            "session.start" => events.push(Event {
                role: "session_meta".to_string(),
                timestamp,
                cwd: data
                    .and_then(|data| data.get("context"))
                    .and_then(|context| context.get("cwd"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                source_tool: "copilot_cli".to_string(),
                ..Event::default()
            }),
            "user.message" | "assistant.message" => events.push(Event {
                role: typ.trim_end_matches(".message").to_string(),
                content: data
                    .and_then(|data| data.get("content").or_else(|| data.get("message")))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                timestamp,
                source_tool: "copilot_cli".to_string(),
                ..Event::default()
            }),
            "tool.execution_start" => events.push(Event {
                role: "assistant".to_string(),
                timestamp,
                tool_calls: vec![ToolCall {
                    id: data
                        .and_then(|data| data.get("toolCallId"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    name: data
                        .and_then(|data| data.get("toolName"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    args: jsonish(data.and_then(|data| data.get("arguments"))),
                }],
                source_tool: "copilot_cli".to_string(),
                ..Event::default()
            }),
            "tool.execution_complete" => events.push(Event {
                role: "tool".to_string(),
                timestamp,
                tool_call_id: data
                    .and_then(|data| data.get("toolCallId"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                is_error: data
                    .and_then(|data| data.get("success"))
                    .and_then(Value::as_bool)
                    == Some(false),
                source_tool: "copilot_cli".to_string(),
                ..Event::default()
            }),
            "session.shutdown" => {
                if let Some(metrics) = data
                    .and_then(|data| data.get("modelMetrics"))
                    .and_then(Value::as_object)
                {
                    for (model, metric) in metrics {
                        if let Some(usage) = metric.get("usage").and_then(usage_from_value) {
                            events.insert(
                                0,
                                Event {
                                    role: "meta".to_string(),
                                    timestamp: timestamp.clone(),
                                    usage,
                                    model_used: model.clone(),
                                    source_tool: "copilot_cli".to_string(),
                                    ..Event::default()
                                },
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
    non_empty(events)
}

fn parse_kimi_wire_jsonl(raw: &str) -> Option<Vec<Event>> {
    let entries = jsonl_objects(raw);
    if !entries.iter().any(|entry| {
        entry
            .get("message")
            .and_then(Value::as_object)
            .is_some_and(|message| message.contains_key("type") && message.contains_key("payload"))
    }) {
        return None;
    }
    let mut events = Vec::new();
    for entry in entries {
        let timestamp = entry
            .get("timestamp")
            .and_then(Value::as_f64)
            .map(|seconds| timestamp_millis((seconds * 1000.0) as i64))
            .unwrap_or_default();
        let Some(message) = entry.get("message").and_then(Value::as_object) else {
            continue;
        };
        let typ = string(message.get("type")).unwrap_or("");
        let payload = message.get("payload").and_then(Value::as_object);
        match typ {
            "TurnBegin" | "SteerInput" => events.push(Event {
                role: "user".to_string(),
                content: payload
                    .and_then(|payload| payload.get("user_input"))
                    .map(|value| jsonish(Some(value)))
                    .unwrap_or_default(),
                timestamp,
                source_tool: "kimi_cli".to_string(),
                ..Event::default()
            }),
            "TextPart" => events.push(Event {
                role: "assistant".to_string(),
                content: payload
                    .and_then(|payload| payload.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                timestamp,
                source_tool: "kimi_cli".to_string(),
                ..Event::default()
            }),
            "ThinkPart" => events.push(Event {
                role: "assistant".to_string(),
                reasoning: payload
                    .and_then(|payload| payload.get("think").or_else(|| payload.get("text")))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                timestamp,
                source_tool: "kimi_cli".to_string(),
                ..Event::default()
            }),
            "ToolCall" | "ToolCallPart" => events.push(Event {
                role: "assistant".to_string(),
                timestamp,
                tool_calls: vec![ToolCall {
                    id: payload
                        .and_then(|payload| payload.get("id"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    name: payload
                        .and_then(|payload| payload.get("function"))
                        .and_then(|function| function.get("name"))
                        .or_else(|| payload.and_then(|payload| payload.get("name")))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    args: jsonish(
                        payload
                            .and_then(|payload| payload.get("function"))
                            .and_then(|function| function.get("arguments"))
                            .or_else(|| payload.and_then(|payload| payload.get("arguments"))),
                    ),
                }],
                source_tool: "kimi_cli".to_string(),
                ..Event::default()
            }),
            "ToolResult" => events.push(Event {
                role: "tool".to_string(),
                content: payload
                    .map(|payload| jsonish(Some(&Value::Object(payload.clone()))))
                    .unwrap_or_default(),
                timestamp,
                source_tool: "kimi_cli".to_string(),
                ..Event::default()
            }),
            "StatusUpdate" => {
                if let Some(usage) = payload
                    .and_then(|payload| payload.get("token_usage"))
                    .and_then(usage_from_value)
                {
                    events.insert(
                        0,
                        Event {
                            role: "meta".to_string(),
                            usage,
                            source_tool: "kimi_cli".to_string(),
                            ..Event::default()
                        },
                    );
                }
            }
            _ => {}
        }
    }
    non_empty(events)
}

fn parse_antigravity_jsonl(raw: &str) -> Option<Vec<Event>> {
    let entries = jsonl_objects(raw);
    if !entries.iter().any(|entry| {
        matches!(
            string(entry.get("type")),
            Some("PLANNER_RESPONSE" | "USER_INPUT" | "CONVERSATION_HISTORY")
        ) && entry.contains_key("step_index")
            && entry.contains_key("created_at")
    }) {
        return None;
    }
    let mut events = Vec::new();
    for entry in entries {
        let typ = string(entry.get("type")).unwrap_or("");
        let timestamp = string(entry.get("created_at")).unwrap_or("").to_string();
        match typ {
            "USER_INPUT" => events.push(Event {
                role: "user".to_string(),
                content: string(entry.get("content")).unwrap_or("").to_string(),
                timestamp,
                source_tool: "antigravity_cli".to_string(),
                ..Event::default()
            }),
            "PLANNER_RESPONSE" => {
                let tool_calls = entry
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|call| {
                        let call = call.as_object()?;
                        Some(ToolCall {
                            name: string(call.get("name")).unwrap_or("").to_string(),
                            args: jsonish(call.get("args")),
                            ..ToolCall::default()
                        })
                    })
                    .collect();
                events.push(Event {
                    role: "assistant".to_string(),
                    content: string(entry.get("content")).unwrap_or("").to_string(),
                    reasoning: string(entry.get("thinking")).unwrap_or("").to_string(),
                    timestamp,
                    tool_calls,
                    source_tool: "antigravity_cli".to_string(),
                    ..Event::default()
                });
            }
            "CONVERSATION_HISTORY" | "SYSTEM_MESSAGE" => {}
            _ => events.push(Event {
                role: "tool".to_string(),
                content: string(entry.get("content")).unwrap_or("").to_string(),
                timestamp,
                is_error: string(entry.get("status")).is_some_and(|status| status != "DONE"),
                source_tool: "antigravity_cli".to_string(),
                ..Event::default()
            }),
        }
    }
    non_empty(events)
}

fn parse_cursor_transcript_jsonl(raw: &str) -> Option<Vec<Event>> {
    let entries = jsonl_objects(raw);
    if entries.is_empty()
        || !entries.iter().all(|entry| {
            entry.contains_key("role") && entry.get("message").and_then(Value::as_object).is_some()
        })
    {
        return None;
    }
    let mut events = Vec::new();
    for entry in entries {
        let role = string(entry.get("role")).unwrap_or("");
        let Some(message) = entry.get("message").and_then(Value::as_object) else {
            continue;
        };
        let mut content = Vec::new();
        let mut tool_calls = Vec::new();
        for block in message
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(block) = block.as_object() else {
                continue;
            };
            match string(block.get("type")).unwrap_or("") {
                "text" => content.push(string(block.get("text")).unwrap_or("").to_string()),
                "tool_use" => tool_calls.push(ToolCall {
                    id: string(block.get("id")).unwrap_or("").to_string(),
                    name: string(block.get("name")).unwrap_or("").to_string(),
                    args: jsonish(block.get("input")),
                }),
                _ => {}
            }
        }
        events.push(Event {
            role: role.to_string(),
            content: content.join("\n"),
            tool_calls,
            source_tool: "cursor".to_string(),
            ..Event::default()
        });
    }
    non_empty(events)
}

fn parse_claude_transcript_jsonl(raw: &str) -> Option<Vec<Event>> {
    let entries = jsonl_objects(raw);
    if !entries.iter().any(|entry| {
        matches!(string(entry.get("type")), Some("tool_use" | "tool_result"))
            && entry.contains_key("timestamp")
            && entry.contains_key("tool_name")
    }) {
        return None;
    }
    let mut events = Vec::new();
    for entry in entries {
        let timestamp = string(entry.get("timestamp")).unwrap_or("").to_string();
        match string(entry.get("type")).unwrap_or("") {
            "user" | "assistant" => events.push(Event {
                role: string(entry.get("type")).unwrap_or("").to_string(),
                content: string(entry.get("content")).unwrap_or("").to_string(),
                timestamp,
                source_tool: "claude_code".to_string(),
                ..Event::default()
            }),
            "tool_use" => events.push(Event {
                role: "assistant".to_string(),
                timestamp,
                tool_calls: vec![ToolCall {
                    name: string(entry.get("tool_name")).unwrap_or("").to_string(),
                    args: jsonish(entry.get("tool_input")),
                    ..ToolCall::default()
                }],
                source_tool: "claude_code".to_string(),
                ..Event::default()
            }),
            "tool_result" => events.push(Event {
                role: "tool".to_string(),
                content: jsonish(entry.get("tool_output")),
                timestamp,
                source_tool: "claude_code".to_string(),
                ..Event::default()
            }),
            _ => {}
        }
    }
    non_empty(events)
}

fn parse_workbuddy_jsonl(raw: &str) -> Option<Vec<Event>> {
    let entries = jsonl_objects(raw);
    if !entries.iter().any(|entry| {
        matches!(
            string(entry.get("type")),
            Some("function_call" | "function_call_result" | "reasoning")
        ) && entry.contains_key("sessionId")
            && entry.contains_key("cwd")
    }) {
        return None;
    }
    let mut events = Vec::new();
    let mut model = "unknown".to_string();
    let mut latest_usage = None;
    for entry in entries {
        if let Some(next) = entry
            .get("providerData")
            .and_then(Value::as_object)
            .and_then(|data| string(data.get("model")))
            .filter(|value| !value.is_empty())
        {
            model = next.to_string();
        }
        let timestamp = entry
            .get("timestamp")
            .and_then(number_as_i64)
            .map(timestamp_millis)
            .unwrap_or_default();
        let cwd = string(entry.get("cwd")).unwrap_or("").to_string();
        match string(entry.get("type")).unwrap_or("") {
            "message" => {
                let role = string(entry.get("role")).unwrap_or("");
                let content = workbuddy_content(entry.get("content"));
                if !content.is_empty() {
                    events.push(Event {
                        role: role.to_string(),
                        content,
                        timestamp,
                        cwd,
                        model_used: model.clone(),
                        source_tool: "workbuddy".to_string(),
                        ..Event::default()
                    });
                }
                latest_usage = workbuddy_usage(&entry).or(latest_usage);
            }
            "reasoning" => {
                let reasoning = workbuddy_content(
                    entry
                        .get("content")
                        .filter(|value| value.as_array().is_some_and(|items| !items.is_empty()))
                        .or_else(|| entry.get("rawContent")),
                );
                if !reasoning.is_empty() {
                    events.push(Event {
                        role: "assistant".to_string(),
                        reasoning,
                        timestamp,
                        cwd,
                        model_used: model.clone(),
                        source_tool: "workbuddy".to_string(),
                        ..Event::default()
                    });
                }
            }
            "function_call" => {
                latest_usage = workbuddy_usage(&entry).or(latest_usage);
                events.push(Event {
                    role: "assistant".to_string(),
                    timestamp,
                    cwd,
                    tool_calls: vec![ToolCall {
                        id: string(entry.get("callId")).unwrap_or("").to_string(),
                        name: string(entry.get("name")).unwrap_or("").to_string(),
                        args: jsonish(entry.get("arguments")),
                    }],
                    model_used: model.clone(),
                    source_tool: "workbuddy".to_string(),
                    ..Event::default()
                });
            }
            "function_call_result" => events.push(Event {
                role: "tool".to_string(),
                content: jsonish(entry.get("output")),
                timestamp,
                cwd,
                tool_call_id: string(entry.get("callId")).unwrap_or("").to_string(),
                is_error: string(entry.get("status")).is_some_and(|status| status != "completed"),
                model_used: model.clone(),
                source_tool: "workbuddy".to_string(),
                ..Event::default()
            }),
            _ => {}
        }
    }
    if let Some(usage) = latest_usage {
        events.insert(
            0,
            Event {
                role: "meta".to_string(),
                usage,
                model_used: model,
                source_tool: "workbuddy".to_string(),
                ..Event::default()
            },
        );
    }
    non_empty(events)
}

fn workbuddy_content(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

fn workbuddy_usage(entry: &Map<String, Value>) -> Option<TokenUsage> {
    let mut usage = entry
        .get("message")
        .and_then(|message| message.get("usage"))
        .or_else(|| entry.get("providerData").and_then(|data| data.get("usage")))
        .and_then(usage_from_value)?;
    let cached = usage.get("cache_read_input_tokens").copied().unwrap_or(0);
    if let Some(input) = usage.get_mut("input_tokens") {
        *input = (*input - cached).max(0);
    }
    Some(usage)
}

fn parse_openclaw_value(value: &Value) -> Option<Vec<Event>> {
    let doc = value.as_object()?;
    if string(doc.get("provider")) != Some("openclaw") {
        return None;
    }
    parse_anthropic_message_wrapper(doc, "openclaw")
}

fn parse_hermes_json_value(value: &Value) -> Option<Vec<Event>> {
    let doc = value.as_object()?;
    if !doc.contains_key("messages")
        || !doc.contains_key("model")
        || !doc.contains_key("session_id")
        || doc.contains_key("provider")
    {
        return None;
    }
    if doc.contains_key("usage") && !doc.contains_key("platform") {
        return None;
    }
    let messages = doc.get("messages").and_then(Value::as_array)?;
    let model = string(doc.get("model")).unwrap_or("").to_string();
    let session_start = string(doc.get("session_start")).unwrap_or("").to_string();
    let session_end = string(doc.get("last_updated")).unwrap_or("").to_string();
    let mut events = Vec::new();

    if let Some(usage) = doc.get("usage").and_then(usage_from_value) {
        events.push(Event {
            role: "meta".to_string(),
            usage,
            model_used: model.clone(),
            source_tool: "hermes_json".to_string(),
            ..Event::default()
        });
    }

    for message in messages {
        let Some(message) = message.as_object() else {
            continue;
        };
        let role = string(message.get("role")).unwrap_or("");
        if role == "tool" {
            continue;
        }
        let mut tool_calls = Vec::new();
        if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                let Some(call) = call.as_object() else {
                    continue;
                };
                let function = call.get("function").and_then(Value::as_object);
                tool_calls.push(ToolCall {
                    id: string(call.get("id")).unwrap_or("").to_string(),
                    name: function
                        .and_then(|function| string(function.get("name")))
                        .unwrap_or("")
                        .to_string(),
                    args: jsonish(function.and_then(|function| function.get("arguments"))),
                });
            }
        }
        events.push(Event {
            role: role.to_string(),
            content: string(message.get("content")).unwrap_or("").to_string(),
            timestamp: string(message.get("timestamp")).unwrap_or("").to_string(),
            reasoning: string(message.get("reasoning"))
                .or_else(|| string(message.get("reasoning_content")))
                .unwrap_or("")
                .to_string(),
            redacted: boolish(message.get("redacted")),
            tool_calls,
            model_used: model.clone(),
            source_tool: "hermes_json".to_string(),
            ..Event::default()
        });
    }

    for message in messages {
        let Some(message) = message.as_object() else {
            continue;
        };
        if string(message.get("role")) != Some("tool") {
            continue;
        }
        events.push(Event {
            role: "tool".to_string(),
            content: string(message.get("content")).unwrap_or("").to_string(),
            timestamp: string(message.get("timestamp")).unwrap_or("").to_string(),
            tool_call_id: string(message.get("tool_call_id"))
                .unwrap_or("")
                .to_string(),
            is_error: boolish(message.get("is_error")),
            source_tool: "hermes_json".to_string(),
            ..Event::default()
        });
    }

    apply_hermes_session_timestamps(&mut events, &session_start, &session_end);
    non_empty(events)
}

fn apply_hermes_session_timestamps(events: &mut [Event], session_start: &str, session_end: &str) {
    if session_start.is_empty() && session_end.is_empty() {
        return;
    }
    if events.iter().any(|event| !event.timestamp.is_empty()) {
        return;
    }
    if !session_start.is_empty() {
        if let Some(first) = events
            .iter_mut()
            .find(|event| event.role != "meta" && event.role != "session_meta")
        {
            first.timestamp = session_start.to_string();
        }
    }
    if !session_end.is_empty() {
        if let Some(last) = events
            .iter_mut()
            .rev()
            .find(|event| event.role != "meta" && event.role != "session_meta")
        {
            last.timestamp = session_end.to_string();
        }
    }
}

fn parse_anthropic_message_wrapper(
    doc: &Map<String, Value>,
    source_tool: &str,
) -> Option<Vec<Event>> {
    let target = doc.get("session").and_then(Value::as_object).unwrap_or(doc);
    let messages = target.get("messages").and_then(Value::as_array)?;
    let model = string(target.get("model"))
        .or_else(|| string(doc.get("model")))
        .unwrap_or("unknown")
        .to_string();
    let mut events = Vec::new();
    if let Some(usage) = target.get("usage").and_then(usage_from_value) {
        events.push(Event {
            role: "meta".to_string(),
            usage,
            model_used: model.clone(),
            source_tool: source_tool.to_string(),
            ..Event::default()
        });
    } else if model != "unknown" {
        events.push(Event {
            role: "meta".to_string(),
            model_used: model.clone(),
            source_tool: source_tool.to_string(),
            ..Event::default()
        });
    }

    for message in messages {
        let Some(message) = message.as_object() else {
            continue;
        };
        anthropic_wrapper_message_events(message, &model, source_tool, &mut events);
    }
    non_empty(events)
}

fn anthropic_wrapper_message_events(
    message: &Map<String, Value>,
    model: &str,
    source_tool: &str,
    events: &mut Vec<Event>,
) {
    let role = string(message.get("role")).unwrap_or("");
    let ts = string(message.get("timestamp")).unwrap_or("").to_string();
    if role == "tool" {
        events.push(Event {
            role: "tool".to_string(),
            content: tool_result_content(message),
            timestamp: ts,
            tool_call_id: string(message.get("tool_call_id"))
                .unwrap_or("")
                .to_string(),
            is_error: boolish(message.get("is_error")),
            source_tool: source_tool.to_string(),
            ..Event::default()
        });
        return;
    }

    match message.get("content") {
        Some(Value::String(text)) => events.push(Event {
            role: role.to_string(),
            content: text.to_string(),
            timestamp: ts.clone(),
            model_used: model.to_string(),
            source_tool: source_tool.to_string(),
            ..Event::default()
        }),
        Some(Value::Array(blocks)) => {
            for block in blocks {
                let Some(block) = block.as_object() else {
                    continue;
                };
                match string(block.get("type")).unwrap_or("") {
                    "text" => events.push(Event {
                        role: role.to_string(),
                        content: string(block.get("text")).unwrap_or("").to_string(),
                        timestamp: ts.clone(),
                        model_used: model.to_string(),
                        source_tool: source_tool.to_string(),
                        ..Event::default()
                    }),
                    "thinking" => events.push(Event {
                        role: "assistant".to_string(),
                        reasoning: string(block.get("thinking")).unwrap_or("").to_string(),
                        redacted: boolish(block.get("redacted")),
                        timestamp: ts.clone(),
                        model_used: model.to_string(),
                        source_tool: source_tool.to_string(),
                        ..Event::default()
                    }),
                    "tool_use" => events.push(Event {
                        role: "assistant".to_string(),
                        timestamp: ts.clone(),
                        tool_calls: vec![ToolCall {
                            id: string(block.get("id")).unwrap_or("").to_string(),
                            name: string(block.get("name")).unwrap_or("").to_string(),
                            args: jsonish(block.get("input").or_else(|| block.get("arguments"))),
                        }],
                        model_used: model.to_string(),
                        source_tool: source_tool.to_string(),
                        ..Event::default()
                    }),
                    "tool_result" => events.push(Event {
                        role: "tool".to_string(),
                        timestamp: ts.clone(),
                        tool_call_id: string(block.get("tool_use_id")).unwrap_or("").to_string(),
                        content: tool_result_content(block),
                        is_error: boolish(block.get("is_error")),
                        source_tool: source_tool.to_string(),
                        ..Event::default()
                    }),
                    _ => {}
                }
            }
        }
        _ => {}
    }

    if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
        let tool_calls = calls
            .iter()
            .filter_map(|call| {
                let call = call.as_object()?;
                let function = call.get("function").and_then(Value::as_object);
                Some(ToolCall {
                    id: string(call.get("id")).unwrap_or("").to_string(),
                    name: function
                        .and_then(|function| string(function.get("name")))
                        .unwrap_or("")
                        .to_string(),
                    args: jsonish(function.and_then(|function| function.get("arguments"))),
                })
            })
            .collect::<Vec<_>>();
        events.push(Event {
            role: role.to_string(),
            timestamp: ts,
            tool_calls,
            model_used: model.to_string(),
            source_tool: source_tool.to_string(),
            ..Event::default()
        });
    }
}

fn is_aider_history(path: &str, trimmed: &str) -> bool {
    if Path::new(path).file_name().and_then(|name| name.to_str()) == Some(".aider.chat.history.md")
    {
        return true;
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return false;
    }
    trimmed.contains("# aider chat started at") && trimmed.contains("#### ")
}

fn parse_aider_chat_history(raw: &str) -> anyhow::Result<Vec<Event>> {
    let mut events = Vec::new();
    let mut role = String::new();
    let mut lines: Vec<String> = Vec::new();
    let mut start_ts = String::new();
    let mut model = "unknown".to_string();
    let mut usage = BTreeMap::new();

    for raw_line in raw.lines() {
        let line = raw_line.trim_end_matches('\r');
        if let Some(value) = line.strip_prefix("# aider chat started at ") {
            flush_aider_event(&mut events, &mut role, &mut lines, &start_ts, &model);
            start_ts = aider_time(value);
            continue;
        }
        if let Some(text) = line.strip_prefix("#### ") {
            if role != "user" {
                flush_aider_event(&mut events, &mut role, &mut lines, &start_ts, &model);
                role = "user".to_string();
            }
            lines.push(text.trim().to_string());
            continue;
        }
        if let Some(text) = line.strip_prefix('>') {
            flush_aider_event(&mut events, &mut role, &mut lines, &start_ts, &model);
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            if let Some(inferred) = infer_aider_model(text) {
                model = inferred;
            }
            merge_aider_usage(&mut usage, text);
            continue;
        }
        if line.trim().is_empty() {
            if !role.is_empty() && !lines.is_empty() {
                lines.push(String::new());
            }
            continue;
        }
        if role != "assistant" {
            flush_aider_event(&mut events, &mut role, &mut lines, &start_ts, &model);
            role = "assistant".to_string();
        }
        lines.push(line.to_string());
    }
    flush_aider_event(&mut events, &mut role, &mut lines, &start_ts, &model);

    if !usage.is_empty() || model != "unknown" || !start_ts.is_empty() {
        let meta = Event {
            role: "meta".to_string(),
            timestamp: start_ts,
            model_used: model,
            source_tool: "aider".to_string(),
            usage,
            ..Event::default()
        };
        events.insert(0, meta);
    }
    if events.is_empty() {
        bail!("aider chat history: no parseable events");
    }
    Ok(events)
}

fn flush_aider_event(
    events: &mut Vec<Event>,
    role: &mut String,
    lines: &mut Vec<String>,
    start_ts: &str,
    model: &str,
) {
    if role.is_empty() {
        lines.clear();
        return;
    }
    let content = lines.join("\n").trim().to_string();
    if !content.is_empty() {
        events.push(Event {
            role: role.clone(),
            content,
            timestamp: start_ts.to_string(),
            model_used: model.to_string(),
            source_tool: "aider".to_string(),
            ..Event::default()
        });
    }
    role.clear();
    lines.clear();
}

fn aider_time(value: &str) -> String {
    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
        .ok()
        .and_then(|ts| ts.and_local_timezone(chrono::Local).single())
        .map(|ts| ts.to_rfc3339_opts(chrono::SecondsFormat::Secs, false))
        .unwrap_or_default()
}

fn infer_aider_model(text: &str) -> Option<String> {
    let parts = text.split_whitespace().collect::<Vec<_>>();
    for pair in parts.windows(2) {
        if pair[0] == "--model" || pair[0] == "-m" {
            return Some(trim_aider_model(pair[1]));
        }
    }
    let lower = text.to_ascii_lowercase();
    for marker in ["model:", "model="] {
        let Some(start) = lower.find(marker) else {
            continue;
        };
        let value = text[start + marker.len()..]
            .split_whitespace()
            .next()
            .unwrap_or("");
        if !value.is_empty() {
            return Some(trim_aider_model(value));
        }
    }
    None
}

fn trim_aider_model(value: &str) -> String {
    value
        .trim_matches(|ch| matches!(ch, '`' | '\'' | '"'))
        .to_string()
}

fn merge_aider_usage(usage: &mut BTreeMap<String, i64>, text: &str) {
    let lower = text.to_ascii_lowercase();
    let Some(tokens_pos) = lower.find("tokens:") else {
        return;
    };
    let rest = &text[tokens_pos + "tokens:".len()..];
    let Some(sent_pos) = rest.to_ascii_lowercase().find(" sent") else {
        return;
    };
    let input = parse_aider_token_count(&rest[..sent_pos]);
    let after_sent = rest[sent_pos + " sent".len()..].trim_start();
    let lower_after = after_sent.to_ascii_lowercase();
    let Some(received_pos) = lower_after.rfind("received") else {
        return;
    };
    let before_received = after_sent[..received_pos].trim();
    let output_part = before_received
        .rsplit(',')
        .next()
        .unwrap_or(before_received);
    let output = parse_aider_token_count(output_part);
    let cache_write = aider_optional_token(before_received, "cache write");
    let cache_hit = aider_optional_token(before_received, "cache hit");

    usage.insert("input_tokens".to_string(), input);
    usage.insert("cache_creation_input_tokens".to_string(), cache_write);
    usage.insert("cache_read_input_tokens".to_string(), cache_hit);
    usage.insert("output_tokens".to_string(), output);
}

fn aider_optional_token(text: &str, label: &str) -> i64 {
    let lower = text.to_ascii_lowercase();
    let Some(label_pos) = lower.find(label) else {
        return 0;
    };
    let prefix = &text[..label_pos];
    parse_aider_token_count(prefix.rsplit(',').next().unwrap_or(prefix))
}

fn parse_aider_token_count(value: &str) -> i64 {
    let mut value = value
        .trim()
        .trim_matches(',')
        .replace(',', "")
        .to_ascii_lowercase();
    if value.is_empty() {
        return 0;
    }
    let multiplier = if value.ends_with('k') {
        value.pop();
        1000.0
    } else {
        1.0
    };
    value
        .trim()
        .parse::<f64>()
        .map(|number| (number * multiplier) as i64)
        .unwrap_or(0)
}

fn is_oh_my_pi_jsonl(raw: &str) -> bool {
    jsonl_objects(raw).iter().any(is_oh_my_pi_session_header)
}

fn is_oh_my_pi_session_header(obj: &Map<String, Value>) -> bool {
    string(obj.get("type")) == Some("session")
        && (obj.contains_key("version")
            || obj.contains_key("cwd")
            || obj.contains_key("titleSource")
            || obj.contains_key("parentSession"))
}

fn parse_oh_my_pi_jsonl(path: &str, raw: &str) -> anyhow::Result<Vec<Event>> {
    let source_tool = pi_source_for_path(path);
    let mut meta_events = Vec::new();
    let mut events = Vec::new();
    let mut model = "unknown".to_string();
    let mut seen_header = false;

    for obj in jsonl_objects(raw) {
        let typ = string(obj.get("type")).unwrap_or("");
        if !seen_header {
            if typ != "session" {
                bail!("oh_my_pi: missing session header");
            }
            if string(obj.get("id")).unwrap_or("").is_empty() {
                bail!("oh_my_pi: invalid session header");
            }
            seen_header = true;
            if let Some(cwd) = string(obj.get("cwd")).filter(|value| !value.is_empty()) {
                meta_events.push(Event {
                    role: "meta".to_string(),
                    cwd: cwd.to_string(),
                    source_tool: source_tool.clone(),
                    ..Event::default()
                });
            }
            continue;
        }

        let ts = string(obj.get("timestamp")).unwrap_or("");
        match typ {
            "message" => {
                let Some(message) = obj.get("message").and_then(Value::as_object) else {
                    continue;
                };
                for event in oh_my_pi_message_events(message, ts, &mut model, &source_tool) {
                    if event.role == "meta" {
                        meta_events.push(event);
                    } else {
                        events.push(event);
                    }
                }
            }
            "custom_message" => {
                let (content, _, _, _) = oh_my_pi_content(obj.get("content"));
                if !content.trim().is_empty() {
                    events.push(Event {
                        role: "user".to_string(),
                        content,
                        timestamp: ts.to_string(),
                        model_used: model.clone(),
                        source_tool: source_tool.clone(),
                        ..Event::default()
                    });
                }
            }
            "model_change" => {
                if let Some(next_model) = string(obj.get("model")).filter(|value| !value.is_empty())
                {
                    model = next_model.to_string();
                    meta_events.push(Event {
                        role: "meta".to_string(),
                        timestamp: ts.to_string(),
                        model_used: model.clone(),
                        source_tool: source_tool.clone(),
                        ..Event::default()
                    });
                }
            }
            "branch_summary" | "compaction" => {
                let content = string(obj.get("summary")).unwrap_or("").trim().to_string();
                if !content.is_empty() {
                    events.push(Event {
                        role: "assistant".to_string(),
                        content,
                        timestamp: ts.to_string(),
                        model_used: model.clone(),
                        source_tool: source_tool.clone(),
                        ..Event::default()
                    });
                }
            }
            _ => {}
        }
    }

    if !seen_header {
        bail!("oh_my_pi: missing session header");
    }
    if events.is_empty() {
        bail!("oh_my_pi: no parseable events");
    }
    meta_events.extend(events);
    Ok(meta_events)
}

fn pi_source_for_path(path: &str) -> String {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    if normalized.contains("/.pi/agent/sessions/") {
        "pi".to_string()
    } else {
        "oh_my_pi".to_string()
    }
}

fn oh_my_pi_message_events(
    message: &Map<String, Value>,
    entry_ts: &str,
    model: &mut String,
    source_tool: &str,
) -> Vec<Event> {
    let role = string(message.get("role")).unwrap_or("");
    let ts = oh_my_pi_timestamp(message.get("timestamp"), entry_ts);
    if let Some(next_model) = string(message.get("model")).filter(|value| !value.is_empty()) {
        *model = next_model.to_string();
    }

    let mut events = Vec::new();
    if let Some(usage) = oh_my_pi_usage(message.get("usage")) {
        events.push(Event {
            role: "meta".to_string(),
            timestamp: ts.clone(),
            usage,
            model_used: model.clone(),
            source_tool: source_tool.to_string(),
            ..Event::default()
        });
    }

    let (content, reasoning, redacted, tool_calls) = oh_my_pi_content(message.get("content"));
    match role {
        "user" => {
            if !content.is_empty() {
                events.push(Event {
                    role: "user".to_string(),
                    content,
                    timestamp: ts,
                    model_used: model.clone(),
                    source_tool: source_tool.to_string(),
                    ..Event::default()
                });
            }
        }
        "developer" => {
            if !content.is_empty() {
                events.push(Event {
                    role: "system".to_string(),
                    content,
                    timestamp: ts,
                    model_used: model.clone(),
                    source_tool: source_tool.to_string(),
                    ..Event::default()
                });
            }
        }
        "assistant" => {
            if !content.is_empty() || !reasoning.is_empty() || !tool_calls.is_empty() {
                events.push(Event {
                    role: "assistant".to_string(),
                    content,
                    reasoning,
                    redacted,
                    tool_calls,
                    timestamp: ts,
                    model_used: model.clone(),
                    source_tool: source_tool.to_string(),
                    ..Event::default()
                });
            }
        }
        "toolResult" => events.push(Event {
            role: "tool".to_string(),
            content,
            timestamp: ts,
            tool_call_id: string(message.get("toolCallId")).unwrap_or("").to_string(),
            is_error: boolish(message.get("isError")),
            model_used: model.clone(),
            source_tool: source_tool.to_string(),
            ..Event::default()
        }),
        _ => {}
    }
    events
}

fn oh_my_pi_content(raw: Option<&Value>) -> (String, String, bool, Vec<ToolCall>) {
    match raw {
        Some(Value::String(text)) => (text.to_string(), String::new(), false, Vec::new()),
        Some(Value::Array(blocks)) => {
            let mut text_parts = Vec::new();
            let mut reasoning_parts = Vec::new();
            let mut tool_calls = Vec::new();
            let mut redacted = false;
            for block in blocks {
                let Some(block) = block.as_object() else {
                    continue;
                };
                match string(block.get("type")).unwrap_or("") {
                    "text" => {
                        if let Some(text) =
                            string(block.get("text")).filter(|value| !value.is_empty())
                        {
                            text_parts.push(text.to_string());
                        }
                    }
                    "thinking" => {
                        if let Some(thinking) =
                            string(block.get("thinking")).filter(|value| !value.is_empty())
                        {
                            reasoning_parts.push(thinking.to_string());
                        }
                    }
                    "redactedThinking" => {
                        redacted = true;
                        if let Some(data) =
                            string(block.get("data")).filter(|value| !value.is_empty())
                        {
                            reasoning_parts.push(data.to_string());
                        }
                    }
                    "toolCall" => {
                        let id = string(block.get("id")).unwrap_or("").to_string();
                        let name = string(block.get("name")).unwrap_or("").to_string();
                        if !id.is_empty() || !name.is_empty() {
                            tool_calls.push(ToolCall {
                                id,
                                name,
                                args: jsonish(block.get("arguments")),
                            });
                        }
                    }
                    "image" => text_parts.push("[image]".to_string()),
                    _ => {}
                }
            }
            (
                text_parts.join("\n"),
                reasoning_parts.join("\n"),
                redacted,
                tool_calls,
            )
        }
        Some(value) => (jsonish(Some(value)), String::new(), false, Vec::new()),
        None => (String::new(), String::new(), false, Vec::new()),
    }
}

fn oh_my_pi_usage(raw: Option<&Value>) -> Option<BTreeMap<String, i64>> {
    let obj = raw.and_then(Value::as_object)?;
    let mut usage = BTreeMap::new();
    let input = sum_numbers(obj, &["input", "input_tokens"]);
    let output = sum_numbers(obj, &["output", "output_tokens"]);
    let cache_read = sum_numbers(obj, &["cacheRead", "cache_read_input_tokens"]);
    let cache_write = sum_numbers(obj, &["cacheWrite", "cache_creation_input_tokens"]);
    if input > 0 {
        usage.insert("input_tokens".to_string(), input);
    }
    if output > 0 {
        usage.insert("output_tokens".to_string(), output);
    }
    if cache_read > 0 {
        usage.insert("cache_read_input_tokens".to_string(), cache_read);
    }
    if cache_write > 0 {
        usage.insert("cache_creation_input_tokens".to_string(), cache_write);
    }
    non_empty_usage(usage)
}

fn oh_my_pi_timestamp(raw: Option<&Value>, fallback: &str) -> String {
    if let Some(ms) = raw.and_then(number_as_i64).filter(|value| *value > 0) {
        return timestamp_millis_nanos(ms);
    }
    string(raw).unwrap_or(fallback).to_string()
}

fn is_qwen_code_jsonl(raw: &str) -> bool {
    jsonl_objects(raw).iter().any(is_qwen_code_event)
}

fn is_qwen_code_value(value: &Value) -> bool {
    match value {
        Value::Object(obj) => is_qwen_code_event(obj) || is_qwen_code_json_output(obj),
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_object)
            .any(is_qwen_code_event),
        _ => false,
    }
}

fn is_qwen_code_event(obj: &Map<String, Value>) -> bool {
    let typ = string(obj.get("type")).unwrap_or("");
    if !matches!(
        typ,
        "system" | "user" | "assistant" | "result" | "stream_event"
    ) {
        return false;
    }
    if obj.contains_key("session_id") {
        return true;
    }
    if obj.contains_key("uuid") {
        return !obj.contains_key("sessionId")
            && (obj.contains_key("message")
                || obj.contains_key("result")
                || obj.contains_key("subtype"));
    }
    false
}

fn is_qwen_code_json_output(obj: &Map<String, Value>) -> bool {
    (obj.contains_key("response") || obj.contains_key("error"))
        && (obj.contains_key("stats") || obj.contains_key("usage"))
}

fn parse_qwen_code_jsonl(raw: &str) -> anyhow::Result<Vec<Event>> {
    let objs = jsonl_objects(raw);
    parse_qwen_code_objects(&objs)
}

fn parse_qwen_code_value(value: &Value) -> anyhow::Result<Vec<Event>> {
    match value {
        Value::Object(obj) if is_qwen_code_json_output(obj) && !is_qwen_code_event(obj) => {
            parse_qwen_code_json_output(obj)
        }
        Value::Object(obj) => parse_qwen_code_objects(std::slice::from_ref(obj)),
        Value::Array(items) => {
            let objs = items
                .iter()
                .filter_map(Value::as_object)
                .cloned()
                .collect::<Vec<_>>();
            parse_qwen_code_objects(&objs)
        }
        _ => bail!("qwen_code: no parseable events"),
    }
}

fn parse_qwen_code_objects(objs: &[Map<String, Value>]) -> anyhow::Result<Vec<Event>> {
    let mut meta_events = Vec::new();
    let mut events = Vec::new();
    let mut model = "unknown".to_string();
    let mut has_assistant = false;
    let mut has_usage = false;

    for obj in objs {
        if !is_qwen_code_event(obj) {
            continue;
        }
        let typ = string(obj.get("type")).unwrap_or("");
        let ts = string(obj.get("timestamp")).unwrap_or("").to_string();
        match typ {
            "system" => {
                if let Some(next_model) = string(obj.get("model")).filter(|value| !value.is_empty())
                {
                    model = next_model.to_string();
                }
                meta_events.push(Event {
                    role: "meta".to_string(),
                    timestamp: ts,
                    model_used: model.clone(),
                    source_tool: "qwen_code".to_string(),
                    ..Event::default()
                });
            }
            "user" => {
                let Some(message) = obj.get("message").and_then(Value::as_object) else {
                    continue;
                };
                events.extend(qwen_message_events(message, &ts, &mut model));
            }
            "assistant" => {
                let Some(message) = obj.get("message").and_then(Value::as_object) else {
                    continue;
                };
                for event in qwen_message_events(message, &ts, &mut model) {
                    if event.role == "meta" {
                        if !event.usage.is_empty() {
                            has_usage = true;
                        }
                        meta_events.push(event);
                    } else {
                        if event.role == "assistant" {
                            has_assistant = true;
                        }
                        events.push(event);
                    }
                }
            }
            "result" => {
                if !has_usage {
                    let usage = qwen_usage(obj.get("usage"))
                        .or_else(|| qwen_stats_usage(obj.get("stats")))
                        .or_else(|| qwen_model_usage(obj.get("modelUsage")));
                    if let Some(usage) = usage {
                        meta_events.push(Event {
                            role: "meta".to_string(),
                            timestamp: ts.clone(),
                            usage,
                            model_used: model.clone(),
                            source_tool: "qwen_code".to_string(),
                            ..Event::default()
                        });
                        has_usage = true;
                    }
                }
                if !has_assistant {
                    let content = string(obj.get("result")).unwrap_or("").trim().to_string();
                    if !content.is_empty() {
                        events.push(Event {
                            role: "assistant".to_string(),
                            content,
                            timestamp: ts,
                            model_used: model.clone(),
                            source_tool: "qwen_code".to_string(),
                            ..Event::default()
                        });
                        has_assistant = true;
                    }
                }
            }
            _ => {}
        }
    }

    if events.is_empty() {
        bail!("qwen_code: no parseable events");
    }
    meta_events.extend(events);
    Ok(meta_events)
}

fn parse_qwen_code_json_output(obj: &Map<String, Value>) -> anyhow::Result<Vec<Event>> {
    let mut events = Vec::new();
    let model = "unknown".to_string();
    if let Some(usage) = qwen_usage(obj.get("usage")).or_else(|| qwen_stats_usage(obj.get("stats")))
    {
        events.push(Event {
            role: "meta".to_string(),
            usage,
            model_used: model.clone(),
            source_tool: "qwen_code".to_string(),
            ..Event::default()
        });
    }
    let content = string(obj.get("response")).unwrap_or("").trim().to_string();
    if !content.is_empty() {
        events.push(Event {
            role: "assistant".to_string(),
            content,
            model_used: model,
            source_tool: "qwen_code".to_string(),
            ..Event::default()
        });
    }
    if events.is_empty() {
        bail!("qwen_code: no parseable events");
    }
    Ok(events)
}

fn qwen_message_events(
    message: &Map<String, Value>,
    fallback_ts: &str,
    model: &mut String,
) -> Vec<Event> {
    if let Some(next_model) = string(message.get("model")).filter(|value| !value.is_empty()) {
        *model = next_model.to_string();
    }
    let ts = string(message.get("timestamp")).unwrap_or(fallback_ts);
    let mut events = Vec::new();
    if let Some(usage) = qwen_usage(message.get("usage")) {
        events.push(Event {
            role: "meta".to_string(),
            timestamp: ts.to_string(),
            usage,
            model_used: model.clone(),
            source_tool: "qwen_code".to_string(),
            ..Event::default()
        });
    }

    let (content, reasoning, redacted, tool_calls) = qwen_content(message.get("content"));
    let tool_results = qwen_tool_result_events(message.get("content"), ts, model);
    let role = string(message.get("role")).unwrap_or("assistant");
    match role {
        "assistant" => {
            if !content.is_empty() || !reasoning.is_empty() || !tool_calls.is_empty() {
                events.push(Event {
                    role: "assistant".to_string(),
                    content,
                    reasoning,
                    redacted,
                    tool_calls,
                    timestamp: ts.to_string(),
                    model_used: model.clone(),
                    source_tool: "qwen_code".to_string(),
                    ..Event::default()
                });
            }
        }
        "user" => {
            if !content.is_empty() {
                events.push(Event {
                    role: "user".to_string(),
                    content,
                    timestamp: ts.to_string(),
                    model_used: model.clone(),
                    source_tool: "qwen_code".to_string(),
                    ..Event::default()
                });
            }
            events.extend(tool_results);
        }
        "tool" | "tool_result" | "toolResult" => events.push(Event {
            role: "tool".to_string(),
            content,
            timestamp: ts.to_string(),
            tool_call_id: string(message.get("tool_call_id"))
                .or_else(|| string(message.get("toolCallId")))
                .unwrap_or("")
                .to_string(),
            is_error: boolish(message.get("is_error")) || boolish(message.get("isError")),
            model_used: model.clone(),
            source_tool: "qwen_code".to_string(),
            ..Event::default()
        }),
        _ => {}
    }
    events
}

fn qwen_content(raw: Option<&Value>) -> (String, String, bool, Vec<ToolCall>) {
    match raw {
        Some(Value::String(text)) => (text.to_string(), String::new(), false, Vec::new()),
        Some(Value::Array(blocks)) => {
            let mut text_parts = Vec::new();
            let mut reasoning_parts = Vec::new();
            let mut tool_calls = Vec::new();
            let mut redacted = false;
            for block in blocks {
                let Some(block) = block.as_object() else {
                    continue;
                };
                match string(block.get("type")).unwrap_or("") {
                    "text" => {
                        if let Some(text) =
                            string(block.get("text")).filter(|value| !value.is_empty())
                        {
                            text_parts.push(text.to_string());
                        }
                    }
                    "thinking" | "reasoning" => {
                        if let Some(text) = string(block.get("thinking"))
                            .or_else(|| string(block.get("text")))
                            .filter(|value| !value.is_empty())
                        {
                            reasoning_parts.push(text.to_string());
                        }
                    }
                    "redacted_thinking" | "redactedThinking" => {
                        redacted = true;
                        if let Some(text) = string(block.get("data"))
                            .or_else(|| string(block.get("text")))
                            .filter(|value| !value.is_empty())
                        {
                            reasoning_parts.push(text.to_string());
                        }
                    }
                    "tool_use" | "toolCall" | "function_call" => {
                        let id = string(block.get("id"))
                            .or_else(|| string(block.get("tool_call_id")))
                            .or_else(|| string(block.get("call_id")))
                            .unwrap_or("")
                            .to_string();
                        let name = string(block.get("name"))
                            .or_else(|| {
                                block
                                    .get("function")
                                    .and_then(Value::as_object)
                                    .and_then(|function| string(function.get("name")))
                            })
                            .unwrap_or("")
                            .to_string();
                        let args = jsonish(block.get("input").or_else(|| block.get("arguments")));
                        if !id.is_empty() || !name.is_empty() {
                            tool_calls.push(ToolCall { id, name, args });
                        }
                    }
                    "tool_result" | "toolResult" => {}
                    "image" => text_parts.push("[image]".to_string()),
                    _ => {}
                }
            }
            (
                text_parts.join("\n"),
                reasoning_parts.join("\n"),
                redacted,
                tool_calls,
            )
        }
        Some(value) => (jsonish(Some(value)), String::new(), false, Vec::new()),
        None => (String::new(), String::new(), false, Vec::new()),
    }
}

fn qwen_tool_result_events(raw: Option<&Value>, ts: &str, model: &str) -> Vec<Event> {
    let Some(blocks) = raw.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut events = Vec::new();
    for block in blocks {
        let Some(block) = block.as_object() else {
            continue;
        };
        let typ = string(block.get("type")).unwrap_or("");
        if typ != "tool_result" && typ != "toolResult" {
            continue;
        }
        events.push(Event {
            role: "tool".to_string(),
            content: tool_result_content(block),
            timestamp: ts.to_string(),
            tool_call_id: string(block.get("tool_use_id"))
                .or_else(|| string(block.get("toolCallId")))
                .unwrap_or("")
                .to_string(),
            is_error: boolish(block.get("is_error")) || boolish(block.get("isError")),
            model_used: model.to_string(),
            source_tool: "qwen_code".to_string(),
            ..Event::default()
        });
    }
    events
}

fn qwen_usage(raw: Option<&Value>) -> Option<BTreeMap<String, i64>> {
    let obj = raw.and_then(Value::as_object)?;
    let mut usage = BTreeMap::new();
    let input = sum_numbers(
        obj,
        &["input_tokens", "prompt_tokens", "input", "promptTokenCount"],
    );
    let output = sum_numbers(
        obj,
        &[
            "output_tokens",
            "completion_tokens",
            "output",
            "candidatesTokenCount",
        ],
    );
    let cache_read = sum_numbers(
        obj,
        &["cache_read_input_tokens", "cacheRead", "cached_tokens"],
    );
    let cache_write = sum_numbers(obj, &["cache_creation_input_tokens", "cacheWrite"]);
    if input > 0 {
        usage.insert("input_tokens".to_string(), input);
    }
    if output > 0 {
        usage.insert("output_tokens".to_string(), output);
    }
    if cache_read > 0 {
        usage.insert("cache_read_input_tokens".to_string(), cache_read);
    }
    if cache_write > 0 {
        usage.insert("cache_creation_input_tokens".to_string(), cache_write);
    }
    non_empty_usage(usage)
}

fn qwen_stats_usage(raw: Option<&Value>) -> Option<BTreeMap<String, i64>> {
    let stats = raw.and_then(Value::as_object)?;
    let models = stats.get("models").and_then(Value::as_object)?;
    let mut usage = BTreeMap::new();
    for model_stats in models.values().filter_map(Value::as_object) {
        let Some(tokens) = model_stats.get("tokens").and_then(Value::as_object) else {
            continue;
        };
        add_usage_value(&mut usage, "input_tokens", tokens.get("input"));
        add_usage_value(&mut usage, "input_tokens", tokens.get("input_tokens"));
        add_usage_value(&mut usage, "output_tokens", tokens.get("output"));
        add_usage_value(&mut usage, "output_tokens", tokens.get("output_tokens"));
        add_usage_value(
            &mut usage,
            "cache_read_input_tokens",
            tokens.get("cacheRead"),
        );
        add_usage_value(
            &mut usage,
            "cache_read_input_tokens",
            tokens.get("cache_read_input_tokens"),
        );
        add_usage_value(
            &mut usage,
            "cache_creation_input_tokens",
            tokens.get("cacheWrite"),
        );
        add_usage_value(
            &mut usage,
            "cache_creation_input_tokens",
            tokens.get("cache_creation_input_tokens"),
        );
    }
    non_empty_usage(usage)
}

fn qwen_model_usage(raw: Option<&Value>) -> Option<BTreeMap<String, i64>> {
    let model_usage = raw.and_then(Value::as_object)?;
    let mut usage = BTreeMap::new();
    for model_stats in model_usage.values().filter_map(Value::as_object) {
        add_usage_value(&mut usage, "input_tokens", model_stats.get("inputTokens"));
        add_usage_value(&mut usage, "input_tokens", model_stats.get("input_tokens"));
        add_usage_value(&mut usage, "output_tokens", model_stats.get("outputTokens"));
        add_usage_value(
            &mut usage,
            "output_tokens",
            model_stats.get("output_tokens"),
        );
        add_usage_value(
            &mut usage,
            "cache_read_input_tokens",
            model_stats.get("cacheReadInputTokens"),
        );
        add_usage_value(
            &mut usage,
            "cache_read_input_tokens",
            model_stats.get("cache_read_input_tokens"),
        );
        add_usage_value(
            &mut usage,
            "cache_creation_input_tokens",
            model_stats.get("cacheCreationInputTokens"),
        );
        add_usage_value(
            &mut usage,
            "cache_creation_input_tokens",
            model_stats.get("cache_creation_input_tokens"),
        );
    }
    non_empty_usage(usage)
}

fn parse_codex_rollout_jsonl(raw: &str) -> Option<Vec<Event>> {
    let mut events = Vec::new();
    let mut model = "unknown".to_string();
    let mut saw_codex = false;
    let mut prev_token_total: Option<BTreeMap<String, i64>> = None;
    for obj in jsonl_objects(raw) {
        let typ = string(obj.get("type")).unwrap_or("");
        let ts = string(obj.get("timestamp")).unwrap_or("").to_string();
        match typ {
            "session_meta" => {
                saw_codex = true;
                let mut cwd = String::new();
                if let Some(payload) = obj.get("payload").and_then(Value::as_object) {
                    if let Some(next_model) = string(payload.get("model")).filter(|m| !m.is_empty())
                    {
                        model = next_model.to_string();
                    }
                    cwd = string(payload.get("cwd")).unwrap_or("").to_string();
                }
                events.push(Event {
                    role: "meta".to_string(),
                    timestamp: ts,
                    cwd,
                    model_used: model.clone(),
                    source_tool: "codex_cli".to_string(),
                    ..Event::default()
                });
            }
            "turn_context" => {
                saw_codex = true;
                if let Some(payload) = obj.get("payload").and_then(Value::as_object) {
                    if let Some(next_model) = string(payload.get("model")).filter(|m| !m.is_empty())
                    {
                        model = next_model.to_string();
                        events.push(Event {
                            role: "meta".to_string(),
                            timestamp: ts,
                            model_used: model.clone(),
                            source_tool: "codex_cli".to_string(),
                            ..Event::default()
                        });
                    }
                }
            }
            "event_msg" => {
                saw_codex = true;
                let Some(payload) = obj.get("payload").and_then(Value::as_object) else {
                    continue;
                };
                if string(payload.get("type")) == Some("token_count") {
                    if let Some((usage, next_total)) =
                        codex_token_count_usage(payload.get("info"), prev_token_total.as_ref())
                    {
                        prev_token_total = next_total;
                        events.push(Event {
                            role: "meta".to_string(),
                            timestamp: ts,
                            usage,
                            model_used: model.clone(),
                            source_tool: "codex_cli".to_string(),
                            ..Event::default()
                        });
                    }
                }
            }
            "response_item" => {
                saw_codex = true;
                let Some(payload) = obj.get("payload").and_then(Value::as_object) else {
                    continue;
                };
                match string(payload.get("type")).unwrap_or("") {
                    "message" => {
                        let mut role = string(payload.get("role")).unwrap_or("").to_string();
                        if role == "developer" {
                            role = "system".to_string();
                        }
                        let mut content = Vec::new();
                        let mut reasoning = Vec::new();
                        match payload.get("content") {
                            Some(Value::String(text)) => content.push(text.clone()),
                            Some(Value::Array(blocks)) => {
                                for block in blocks {
                                    let Some(block) = block.as_object() else {
                                        continue;
                                    };
                                    match string(block.get("type")).unwrap_or("") {
                                        "input_text" | "output_text" | "text" => {
                                            if let Some(text) =
                                                string(block.get("text")).filter(|t| !t.is_empty())
                                            {
                                                content.push(text.to_string());
                                            }
                                        }
                                        "reasoning" | "thinking" => {
                                            if let Some(text) =
                                                string(block.get("text")).filter(|t| !t.is_empty())
                                            {
                                                reasoning.push(text.to_string());
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            _ => {}
                        }
                        for item in reasoning {
                            events.push(Event {
                                role: role.clone(),
                                reasoning: item,
                                timestamp: ts.clone(),
                                model_used: model.clone(),
                                source_tool: "codex_cli".to_string(),
                                ..Event::default()
                            });
                        }
                        events.push(Event {
                            role,
                            content: content.join("\n"),
                            timestamp: ts,
                            model_used: model.clone(),
                            source_tool: "codex_cli".to_string(),
                            ..Event::default()
                        });
                    }
                    "function_call" => {
                        events.push(Event {
                            role: "assistant".to_string(),
                            timestamp: ts,
                            tool_calls: vec![ToolCall {
                                id: string(payload.get("call_id")).unwrap_or("").to_string(),
                                name: string(payload.get("name")).unwrap_or("").to_string(),
                                args: jsonish(
                                    payload.get("arguments").or_else(|| payload.get("input")),
                                ),
                            }],
                            model_used: model.clone(),
                            source_tool: "codex_cli".to_string(),
                            ..Event::default()
                        });
                    }
                    "function_call_output" | "function_call_result" => {
                        events.push(Event {
                            role: "tool".to_string(),
                            timestamp: ts,
                            tool_call_id: string(payload.get("call_id")).unwrap_or("").to_string(),
                            content: jsonish(payload.get("output")),
                            source_tool: "codex_cli".to_string(),
                            ..Event::default()
                        });
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    if saw_codex {
        non_empty(events)
    } else {
        None
    }
}

fn codex_token_count_usage(
    raw_info: Option<&Value>,
    prev_total: Option<&TokenUsage>,
) -> Option<(TokenUsage, Option<TokenUsage>)> {
    let info = raw_info?.as_object()?;
    let total = token_usage_map(info.get("total_token_usage"));
    let (counts, next_total) = if !total.is_empty() {
        (token_usage_delta(&total, prev_total), Some(total))
    } else {
        (
            token_usage_map(info.get("last_token_usage")),
            prev_total.cloned(),
        )
    };
    if counts.is_empty() || !usage_has_values(&counts) {
        return None;
    }

    let mut cache_read = counts.get("cached_input_tokens").copied().unwrap_or(0);
    if cache_read == 0 {
        cache_read = counts.get("cache_read_input_tokens").copied().unwrap_or(0);
    }
    let cache_write = counts
        .get("cache_creation_input_tokens")
        .copied()
        .unwrap_or(0);
    let input = (counts.get("input_tokens").copied().unwrap_or(0) - cache_read).max(0);
    let output = counts.get("output_tokens").copied().unwrap_or(0)
        + counts.get("reasoning_output_tokens").copied().unwrap_or(0);

    let mut usage = BTreeMap::new();
    usage.insert("input_tokens".to_string(), input);
    usage.insert("output_tokens".to_string(), output);
    usage.insert("cache_creation_input_tokens".to_string(), cache_write);
    usage.insert("cache_read_input_tokens".to_string(), cache_read);
    Some((usage, next_total))
}

fn token_usage_map(raw: Option<&Value>) -> TokenUsage {
    let Some(obj) = raw.and_then(Value::as_object) else {
        return BTreeMap::new();
    };
    [
        "input_tokens",
        "output_tokens",
        "cached_input_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
        "reasoning_output_tokens",
    ]
    .iter()
    .filter_map(|key| {
        obj.get(*key)
            .and_then(number_as_i64)
            .filter(|value| *value > 0)
            .map(|value| ((*key).to_string(), value))
    })
    .collect()
}

fn token_usage_delta(cur: &TokenUsage, prev: Option<&TokenUsage>) -> TokenUsage {
    let Some(prev) = prev else {
        return cur.clone();
    };
    [
        "input_tokens",
        "output_tokens",
        "cached_input_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
        "reasoning_output_tokens",
    ]
    .iter()
    .filter_map(|key| {
        let delta = cur.get(*key).copied().unwrap_or(0) - prev.get(*key).copied().unwrap_or(0);
        (delta > 0).then(|| ((*key).to_string(), delta))
    })
    .collect()
}

fn parse_claude_code_jsonl(raw: &str) -> Option<Vec<Event>> {
    let mut events = Vec::new();
    let mut model = "unknown".to_string();
    let mut saw_claude = false;
    let mut seen_usage_snapshots = BTreeSet::new();
    let mut cwd = String::new();
    for obj in jsonl_objects(raw) {
        let typ = string(obj.get("type")).unwrap_or("");
        if cwd.is_empty() {
            if let Some(body_cwd) = string(obj.get("cwd")).filter(|value| !value.is_empty()) {
                cwd = body_cwd.to_string();
                events.push(Event {
                    role: "session_meta".to_string(),
                    cwd: cwd.clone(),
                    model_used: model.clone(),
                    source_tool: "claude_code".to_string(),
                    ..Event::default()
                });
            }
        }
        match typ {
            "user" => {
                saw_claude = true;
                let ts = string(obj.get("timestamp")).unwrap_or("").to_string();
                let Some(message) = obj.get("message").and_then(Value::as_object) else {
                    continue;
                };
                claude_user_content_events(message.get("content"), &ts, &model, &mut events);
            }
            "assistant" => {
                saw_claude = true;
                let ts = string(obj.get("timestamp")).unwrap_or("").to_string();
                let Some(message) = obj.get("message").and_then(Value::as_object) else {
                    continue;
                };
                if model == "unknown" {
                    if let Some(next_model) = string(message.get("model")).filter(|m| !m.is_empty())
                    {
                        model = next_model.to_string();
                    }
                }
                if let Some(usage_value) = message.get("usage") {
                    let message_id = string(message.get("id")).unwrap_or("");
                    let usage_key = if message_id.is_empty() {
                        String::new()
                    } else {
                        serde_json::to_string(usage_value)
                            .map(|usage| format!("{message_id}:{usage}"))
                            .unwrap_or_default()
                    };
                    if usage_key.is_empty() || seen_usage_snapshots.insert(usage_key) {
                        if let Some(usage) = usage_from_value(usage_value) {
                            events.push(Event {
                                role: "meta".to_string(),
                                timestamp: ts.clone(),
                                usage,
                                model_used: model.clone(),
                                source_tool: "claude_code".to_string(),
                                ..Event::default()
                            });
                        }
                    }
                }
                let mut assistant_parts = Vec::new();
                let mut reasoning_parts = Vec::new();
                let mut tool_calls = Vec::new();
                for block in message
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(block) = block.as_object() else {
                        continue;
                    };
                    match string(block.get("type")).unwrap_or("") {
                        "text" => {
                            if let Some(text) = string(block.get("text")).filter(|t| !t.is_empty())
                            {
                                assistant_parts.push(text.to_string());
                            }
                        }
                        "thinking" => {
                            if let Some(text) =
                                string(block.get("thinking")).filter(|t| !t.is_empty())
                            {
                                reasoning_parts.push(text.to_string());
                            }
                        }
                        "tool_use" => tool_calls.push(ToolCall {
                            id: string(block.get("id")).unwrap_or("").to_string(),
                            name: string(block.get("name")).unwrap_or("").to_string(),
                            args: jsonish(block.get("input").or_else(|| block.get("arguments"))),
                        }),
                        "tool_result" => events.push(Event {
                            role: "tool".to_string(),
                            timestamp: ts.clone(),
                            tool_call_id: string(block.get("tool_use_id"))
                                .unwrap_or("")
                                .to_string(),
                            content: tool_result_content(block),
                            is_error: block
                                .get("is_error")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            source_tool: "claude_code".to_string(),
                            ..Event::default()
                        }),
                        _ => {}
                    }
                }
                if !assistant_parts.is_empty()
                    || !reasoning_parts.is_empty()
                    || !tool_calls.is_empty()
                {
                    events.push(Event {
                        role: "assistant".to_string(),
                        content: assistant_parts.join("\n"),
                        reasoning: reasoning_parts.join("\n"),
                        timestamp: ts,
                        tool_calls,
                        model_used: model.clone(),
                        source_tool: "claude_code".to_string(),
                        ..Event::default()
                    });
                }
            }
            _ => {}
        }
    }
    if saw_claude {
        non_empty(events)
    } else {
        None
    }
}

fn claude_user_content_events(
    content: Option<&Value>,
    ts: &str,
    model: &str,
    events: &mut Vec<Event>,
) {
    match content {
        Some(Value::String(text)) => events.push(Event {
            role: "user".to_string(),
            content: text.to_string(),
            timestamp: ts.to_string(),
            model_used: model.to_string(),
            source_tool: "claude_code".to_string(),
            ..Event::default()
        }),
        Some(Value::Array(blocks)) => {
            for block in blocks {
                let Some(block) = block.as_object() else {
                    continue;
                };
                match string(block.get("type")).unwrap_or("") {
                    "text" => events.push(Event {
                        role: "user".to_string(),
                        content: string(block.get("text")).unwrap_or("").to_string(),
                        timestamp: ts.to_string(),
                        model_used: model.to_string(),
                        source_tool: "claude_code".to_string(),
                        ..Event::default()
                    }),
                    "tool_result" => events.push(Event {
                        role: "tool".to_string(),
                        timestamp: ts.to_string(),
                        tool_call_id: string(block.get("tool_use_id")).unwrap_or("").to_string(),
                        content: tool_result_content(block),
                        is_error: block
                            .get("is_error")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        source_tool: "claude_code".to_string(),
                        ..Event::default()
                    }),
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn parse_copilot_jsonl(raw: &str) -> Option<Vec<Event>> {
    let mut events = Vec::new();
    let mut model = "unknown".to_string();
    let mut saw_copilot = false;
    for span in jsonl_objects(raw) {
        let name = string(span.get("name")).unwrap_or("");
        if name.is_empty() || !span.contains_key("traceId") {
            continue;
        }
        saw_copilot = true;
        if let Some(next_model) = copilot_string_attr(&span, "gen_ai.request.model") {
            model = next_model;
        }
        let ts = copilot_timestamp(span.get("startTimeUnixNano"));
        let usage = copilot_usage(&span);
        match name {
            "chat.completion" => {
                let content = copilot_span_content(&span);
                if !content.is_empty() {
                    events.push(Event {
                        role: "assistant".to_string(),
                        content,
                        timestamp: ts.clone(),
                        model_used: model.clone(),
                        source_tool: "copilot_cli".to_string(),
                        ..Event::default()
                    });
                }
                if usage_has_values(&usage) {
                    events.push(Event {
                        role: "meta".to_string(),
                        usage,
                        model_used: model.clone(),
                        source_tool: "copilot_cli".to_string(),
                        ..Event::default()
                    });
                }
            }
            "tool.call" => events.push(Event {
                role: "assistant".to_string(),
                timestamp: ts,
                tool_calls: vec![ToolCall {
                    id: copilot_string_attr(&span, "tool.call.id")
                        .or_else(|| string(span.get("spanId")).map(str::to_string))
                        .unwrap_or_default(),
                    name: copilot_string_attr(&span, "tool.name").unwrap_or_default(),
                    ..ToolCall::default()
                }],
                model_used: model.clone(),
                source_tool: "copilot_cli".to_string(),
                ..Event::default()
            }),
            "tool.result" => events.push(Event {
                role: "tool".to_string(),
                content: copilot_span_content(&span),
                timestamp: ts,
                tool_call_id: copilot_string_attr(&span, "tool.call.id")
                    .or_else(|| string(span.get("parentSpanId")).map(str::to_string))
                    .unwrap_or_default(),
                is_error: copilot_bool_attr(&span, "tool.result.is_error"),
                model_used: model.clone(),
                source_tool: "copilot_cli".to_string(),
                ..Event::default()
            }),
            _ => {
                let content = copilot_span_content(&span);
                if !content.is_empty() {
                    events.push(Event {
                        role: "assistant".to_string(),
                        content,
                        timestamp: ts,
                        model_used: model.clone(),
                        source_tool: "copilot_cli".to_string(),
                        ..Event::default()
                    });
                }
            }
        }
    }
    if saw_copilot {
        non_empty(events)
    } else {
        None
    }
}

fn parse_kimi_value(value: &Value) -> Option<Vec<Event>> {
    let doc = value.as_object()?;
    if !doc.contains_key("messages") || !doc.contains_key("model") {
        return None;
    }
    let model = string(doc.get("model")).unwrap_or("unknown").to_string();
    let mut events = Vec::new();
    if let Some(usage) = doc.get("usage").and_then(usage_from_value) {
        events.push(Event {
            role: "meta".to_string(),
            usage,
            model_used: model.clone(),
            source_tool: "kimi_cli".to_string(),
            ..Event::default()
        });
    }
    if let Some(usage) = doc
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get("usage"))
        .and_then(usage_from_value)
    {
        events.push(Event {
            role: "meta".to_string(),
            usage,
            model_used: model.clone(),
            source_tool: "kimi_cli".to_string(),
            ..Event::default()
        });
    }
    for message in doc.get("messages")?.as_array()? {
        let Some(message) = message.as_object() else {
            continue;
        };
        kimi_message_events(message, &model, &mut events);
    }
    non_empty(events)
}

fn parse_messages_value(value: &Value, source_tool: &str) -> Option<Vec<Event>> {
    let doc = value.as_object()?;
    let messages = doc.get("messages")?.as_array()?;
    let model = string(doc.get("model")).unwrap_or("unknown").to_string();
    let mut events = Vec::new();
    for message in messages {
        if let Some(mut event) = event_from_message(message, &model) {
            event.source_tool = source_tool.to_string();
            events.push(event);
        }
    }
    non_empty(events)
}

fn event_from_message(message: &Value, model: &str) -> Option<Event> {
    let obj = message.as_object()?;
    let role = obj
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let timestamp = obj
        .get("timestamp")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut content = String::new();
    let mut reasoning = String::new();
    if let Some(value) = obj.get("content") {
        match value {
            Value::String(text) => content = text.clone(),
            Value::Array(blocks) => {
                for block in blocks {
                    let Some(block) = block.as_object() else {
                        continue;
                    };
                    match block.get("type").and_then(Value::as_str).unwrap_or("") {
                        "text" => {
                            if let Some(text) = block.get("text").and_then(Value::as_str) {
                                if !content.is_empty() {
                                    content.push('\n');
                                }
                                content.push_str(text);
                            }
                        }
                        "thinking" => {
                            if let Some(text) = block
                                .get("thinking")
                                .or_else(|| block.get("text"))
                                .and_then(Value::as_str)
                            {
                                reasoning.push_str(text);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    if reasoning.is_empty() {
        reasoning = obj
            .get("reasoning")
            .or_else(|| obj.get("reasoning_content"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
    }
    let mut tool_calls = Vec::new();
    if let Some(calls) = obj.get("tool_calls").and_then(Value::as_array) {
        for call in calls {
            if let Some(call) = call.as_object() {
                let mut tool_call = ToolCall {
                    id: string(call.get("id")).unwrap_or("").to_string(),
                    ..ToolCall::default()
                };
                if let Some(function) = call.get("function").and_then(Value::as_object) {
                    tool_call.name = string(function.get("name")).unwrap_or("").to_string();
                    tool_call.args = jsonish(function.get("arguments"));
                }
                if !tool_call.name.is_empty() || !tool_call.args.is_empty() {
                    tool_calls.push(tool_call);
                }
            }
        }
    }
    Some(Event {
        role,
        content,
        timestamp,
        reasoning,
        tool_calls,
        tool_call_id: string(obj.get("tool_call_id")).unwrap_or("").to_string(),
        is_error: obj
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        model_used: model.to_string(),
        ..Event::default()
    })
}

fn parse_cursor_export(value: &Value) -> Option<Vec<Event>> {
    let mut events = Vec::new();
    if let Some(doc) = value.as_object() {
        add_cursor_prompts(
            first_value(doc, &["aiService.prompts", "prompts"]),
            &mut events,
        );
        add_cursor_generations(
            first_value(doc, &["aiService.generations", "generations"]),
            &mut events,
        );
        add_cursor_composers(
            first_value(doc, &["composer.composerData", "composerData"]).or(Some(value)),
            &mut events,
        );
    } else if value.is_array() {
        add_cursor_prompts(Some(value), &mut events);
        add_cursor_generations(Some(value), &mut events);
    }
    non_empty(events)
}

fn add_cursor_prompts(value: Option<&Value>, events: &mut Vec<Event>) {
    for item in cursor_array(value) {
        let Some(item) = item.as_object() else {
            continue;
        };
        let Some(text) = string(item.get("text")) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        events.push(Event {
            role: "user".to_string(),
            content: text.to_string(),
            source_tool: "cursor".to_string(),
            ..Event::default()
        });
    }
}

fn add_cursor_generations(value: Option<&Value>, events: &mut Vec<Event>) {
    for item in cursor_array(value) {
        let Some(item) = item.as_object() else {
            continue;
        };
        let content = string(item.get("textDescription"))
            .or_else(|| string(item.get("description")))
            .or_else(|| string(item.get("text")))
            .or_else(|| string(item.get("type")))
            .unwrap_or("");
        if content.is_empty() {
            continue;
        }
        events.push(Event {
            role: "assistant".to_string(),
            content: content.to_string(),
            timestamp: cursor_timestamp(item.get("unixMs")),
            source_tool: "cursor".to_string(),
            ..Event::default()
        });
    }
}

fn add_cursor_composers(value: Option<&Value>, events: &mut Vec<Event>) {
    let Some(value) = cursor_object(value) else {
        return;
    };
    for item in cursor_array(value.get("allComposers")) {
        let Some(item) = item.as_object() else {
            continue;
        };
        let fallback_ts = cursor_timestamp(first_value(item, &["lastUpdatedAt", "createdAt"]));
        let msg_events = cursor_composer_message_events(item, &fallback_ts);
        if !msg_events.is_empty() {
            events.extend(msg_events);
            continue;
        }
        let mut content = string(item.get("name")).unwrap_or("").to_string();
        let subtitle = string(item.get("subtitle")).unwrap_or("");
        if !subtitle.is_empty() && subtitle != content {
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(subtitle);
        }
        if content.is_empty() {
            content = string(item.get("type")).unwrap_or("").to_string();
        }
        if !content.is_empty() {
            events.push(Event {
                role: "assistant".to_string(),
                content,
                timestamp: fallback_ts,
                source_tool: "cursor".to_string(),
                ..Event::default()
            });
        }
    }
}

fn cursor_composer_message_events(composer: &Map<String, Value>, fallback_ts: &str) -> Vec<Event> {
    let mut messages = Vec::new();
    if let Some(conversation) = composer.get("conversation").and_then(Value::as_object) {
        messages.extend(cursor_array(conversation.get("messages")));
    }
    messages.extend(cursor_array(composer.get("messages")));

    let mut events = Vec::new();
    for message in messages {
        let Some(message) = message.as_object() else {
            continue;
        };
        let role = cursor_role(
            string(message.get("role"))
                .or_else(|| string(message.get("speaker")))
                .or_else(|| string(message.get("type")))
                .unwrap_or(""),
        );
        let content = cursor_text(
            first_value(
                message,
                &["text", "content", "message", "markdown", "rawText"],
            )
            .unwrap_or(&Value::Null),
        );
        if role.is_empty() || content.is_empty() {
            continue;
        }
        let mut timestamp = cursor_timestamp(first_value(
            message,
            &["unixMs", "timestamp", "createdAt", "lastUpdatedAt"],
        ));
        if timestamp.is_empty() {
            timestamp = fallback_ts.to_string();
        }
        events.push(Event {
            role,
            content,
            timestamp,
            source_tool: "cursor".to_string(),
            ..Event::default()
        });
    }
    events
}

fn parse_gemini_value(value: &Value) -> Option<Vec<Event>> {
    let mut events = Vec::new();
    let mut model = "unknown".to_string();
    parse_gemini_object(value, &mut model, &mut events);
    if events.is_empty() {
        if let Some(arr) = value.as_array() {
            parse_gemini_array(arr, "", &model, &mut events);
        }
    }
    non_empty(events)
}

fn parse_gemini_object(value: &Value, model: &mut String, events: &mut Vec<Event>) {
    let Some(obj) = value.as_object() else {
        return;
    };
    for key in ["modelVersion", "model", "modelId"] {
        if let Some(value) = string(obj.get(key)) {
            if !value.is_empty() {
                *model = value.to_string();
            }
        }
    }
    for key in ["usageMetadata", "usage", "tokenUsage"] {
        if let Some(usage) = obj.get(key).and_then(gemini_usage) {
            events.push(Event {
                role: "meta".to_string(),
                usage,
                model_used: model.clone(),
                source_tool: "gemini_cli".to_string(),
                ..Event::default()
            });
        }
    }
    let fallback_ts = string(obj.get("timestamp")).unwrap_or("");
    if let Some(contents) = obj.get("contents").and_then(Value::as_array) {
        parse_gemini_array(contents, fallback_ts, model, events);
    }
    for key in [
        "history",
        "messages",
        "conversation",
        "clientHistory",
        "chatHistory",
    ] {
        if let Some(contents) = obj.get(key).and_then(Value::as_array) {
            parse_gemini_array(contents, fallback_ts, model, events);
        }
    }
    if let Some(candidates) = obj.get("candidates").and_then(Value::as_array) {
        for candidate in candidates {
            if let Some(content) = candidate.get("content").and_then(Value::as_object) {
                parse_gemini_content_object(content, fallback_ts, model, events);
            }
        }
    }
    if obj.contains_key("parts") {
        parse_gemini_content_object(obj, fallback_ts, model, events);
    }
    for key in ["checkpoint", "session", "chat"] {
        if let Some(nested) = obj.get(key) {
            parse_gemini_object(nested, model, events);
        }
    }
}

fn parse_gemini_array(items: &[Value], fallback_ts: &str, model: &str, events: &mut Vec<Event>) {
    for item in items {
        if let Some(item) = item.as_object() {
            parse_gemini_content_object(item, fallback_ts, model, events);
        }
    }
}

fn parse_gemini_content_object(
    obj: &Map<String, Value>,
    fallback_ts: &str,
    model: &str,
    events: &mut Vec<Event>,
) {
    let role = gemini_role(string(obj.get("role")).unwrap_or(""));
    let ts = string(obj.get("timestamp")).unwrap_or(fallback_ts);
    let Some(parts) = obj.get("parts").and_then(Value::as_array) else {
        return;
    };
    for part in parts {
        let Some(part) = part.as_object() else {
            continue;
        };
        if let Some(text) = string(part.get("text")) {
            if part
                .get("thought")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                events.push(Event {
                    role: "assistant".to_string(),
                    reasoning: text.to_string(),
                    timestamp: ts.to_string(),
                    model_used: model.to_string(),
                    source_tool: "gemini_cli".to_string(),
                    ..Event::default()
                });
            } else {
                events.push(Event {
                    role: role.clone(),
                    content: text.to_string(),
                    timestamp: ts.to_string(),
                    model_used: model.to_string(),
                    source_tool: "gemini_cli".to_string(),
                    ..Event::default()
                });
            }
        }
        if let Some(function_call) = part.get("functionCall").and_then(Value::as_object) {
            let name = string(function_call.get("name")).unwrap_or("").to_string();
            let args = jsonish(function_call.get("args"));
            if !name.is_empty() || !args.is_empty() {
                events.push(Event {
                    role: "assistant".to_string(),
                    timestamp: ts.to_string(),
                    tool_calls: vec![ToolCall {
                        name,
                        args,
                        ..ToolCall::default()
                    }],
                    model_used: model.to_string(),
                    source_tool: "gemini_cli".to_string(),
                    ..Event::default()
                });
            }
        }
        if let Some(function_response) = part.get("functionResponse").and_then(Value::as_object) {
            let name = string(function_response.get("name"))
                .unwrap_or("")
                .to_string();
            let content = jsonish(function_response.get("response"));
            if !name.is_empty() || !content.is_empty() {
                events.push(Event {
                    role: "tool".to_string(),
                    content,
                    timestamp: ts.to_string(),
                    tool_call_id: name,
                    source_tool: "gemini_cli".to_string(),
                    ..Event::default()
                });
            }
        }
    }
}

#[derive(Debug)]
struct OpenCodeRecord {
    path: PathBuf,
    doc: Map<String, Value>,
}

fn parse_opencode_storage_value(path: &str, value: &Value) -> Option<Vec<Event>> {
    let doc = value.as_object()?;
    if !is_opencode_storage_session_doc(path, doc) {
        return None;
    }
    parse_opencode_storage_session(path, doc).ok()
}

fn is_opencode_storage_session_doc(path: &str, doc: &Map<String, Value>) -> bool {
    is_opencode_storage_session_file(path)
        && !string(doc.get("id")).unwrap_or("").is_empty()
        && !string(doc.get("projectID")).unwrap_or("").is_empty()
}

fn is_opencode_storage_session_file(path: &str) -> bool {
    let Some(rel) = opencode_storage_rel(path) else {
        return false;
    };
    let parts = rel.split('/').collect::<Vec<_>>();
    parts.len() == 3 && parts[0] == "session" && parts[2].ends_with(".json")
}

fn opencode_storage_rel(path: &str) -> Option<String> {
    if let Ok(root) = std::env::var("OPENCODE_DATA_DIR") {
        if !root.is_empty() {
            if let Some(rel) = rel_from_root(Path::new(&root), Path::new(path)) {
                return Some(rel);
            }
        }
    }

    let clean = path_slash(Path::new(path));
    let marker = "/opencode/storage";
    let idx = clean.rfind(marker)?;
    let start = idx + marker.len();
    if clean.len() == start {
        return Some(String::new());
    }
    if clean.as_bytes().get(start) != Some(&b'/') {
        return None;
    }
    Some(clean[start + 1..].trim_matches('/').to_string())
}

fn rel_from_root(root: &Path, path: &Path) -> Option<String> {
    let abs_root = std::fs::canonicalize(root).ok()?;
    let abs_path = std::fs::canonicalize(path).ok()?;
    let rel = abs_path.strip_prefix(abs_root).ok()?;
    if rel.as_os_str().is_empty() {
        Some(String::new())
    } else {
        Some(path_slash(rel))
    }
}

fn parse_opencode_storage_session(
    path: &str,
    session: &Map<String, Value>,
) -> anyhow::Result<Vec<Event>> {
    let session_id = string(session.get("id")).unwrap_or("");
    if session_id.is_empty() {
        bail!("opencode: missing session id");
    }
    let storage_root = opencode_storage_root_from_session_file(path)
        .with_context(|| format!("opencode: unsupported storage path {path}"))?;
    let mut messages = read_opencode_records(&storage_root.join("message").join(session_id));
    if messages.is_empty() {
        bail!("opencode: no messages found for session {}", session_id);
    }
    sort_opencode_records(&mut messages);

    let mut model = opencode_session_model(session);
    let mut usage = BTreeMap::new();
    let mut body = Vec::new();
    for msg in messages {
        if model == "unknown" {
            let msg_model = opencode_message_model(&msg.doc);
            if !msg_model.is_empty() {
                model = msg_model;
            }
        }
        let message_had_usage = add_opencode_tokens(&mut usage, msg.doc.get("tokens"));
        let (events, part_usage) =
            parse_opencode_message(&storage_root, &msg.doc, &model, message_had_usage);
        add_usage(&mut usage, &part_usage);
        body.extend(events);
    }
    if body.is_empty() {
        bail!("opencode: no parseable events for session {}", session_id);
    }

    let mut events = Vec::new();
    if model != "unknown" || usage_has_values(&usage) {
        events.push(Event {
            role: "meta".to_string(),
            model_used: model,
            source_tool: "opencode".to_string(),
            usage: if usage_has_values(&usage) {
                usage
            } else {
                BTreeMap::new()
            },
            ..Event::default()
        });
    }
    events.extend(body);
    Ok(events)
}

fn opencode_storage_root_from_session_file(path: &str) -> Option<PathBuf> {
    if !is_opencode_storage_session_file(path) {
        return None;
    }
    Path::new(path)
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}

fn read_opencode_records(dir: &Path) -> Vec<OpenCodeRecord> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut records = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() || path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(Value::Object(doc)) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        records.push(OpenCodeRecord { path, doc });
    }
    records
}

fn sort_opencode_records(records: &mut [OpenCodeRecord]) {
    records.sort_by(
        |a, b| match (opencode_record_time(&a.doc), opencode_record_time(&b.doc)) {
            (Some(left), Some(right)) if left != right => left.cmp(&right),
            _ => a.path.cmp(&b.path),
        },
    );
}

fn parse_opencode_message(
    storage_root: &Path,
    msg: &Map<String, Value>,
    model: &str,
    message_had_usage: bool,
) -> (Vec<Event>, BTreeMap<String, i64>) {
    let role = string(msg.get("role")).unwrap_or("");
    let ts = opencode_time_from_map(msg.get("time"), &["created", "start"]);
    let msg_id = string(msg.get("id")).unwrap_or("");
    let mut parts = read_opencode_records(&storage_root.join("part").join(msg_id));
    sort_opencode_records(&mut parts);

    let mut events = Vec::new();
    let mut part_usage = BTreeMap::new();
    for record in parts.iter() {
        let part = &record.doc;
        let part_ts = opencode_part_timestamp(part, &ts);
        match string(part.get("type")).unwrap_or("") {
            "text" => {
                let text = string(part.get("text")).unwrap_or("");
                if !text.is_empty() && (role == "user" || role == "assistant") {
                    events.push(Event {
                        role: role.to_string(),
                        content: text.to_string(),
                        timestamp: part_ts,
                        model_used: model.to_string(),
                        source_tool: "opencode".to_string(),
                        ..Event::default()
                    });
                }
            }
            "reasoning" => {
                let text = string(part.get("text")).unwrap_or("");
                if !text.is_empty() {
                    events.push(Event {
                        role: "assistant".to_string(),
                        reasoning: text.to_string(),
                        timestamp: part_ts,
                        model_used: model.to_string(),
                        source_tool: "opencode".to_string(),
                        ..Event::default()
                    });
                }
            }
            "tool" => events.extend(opencode_tool_events(part, &part_ts, model)),
            "step-finish" if !message_had_usage => {
                add_opencode_tokens(&mut part_usage, part.get("tokens"));
            }
            _ => {}
        }
    }
    if parts.is_empty() {
        let text = string(msg.get("content")).unwrap_or("");
        if !text.is_empty() && (role == "user" || role == "assistant") {
            events.push(Event {
                role: role.to_string(),
                content: text.to_string(),
                timestamp: ts,
                model_used: model.to_string(),
                source_tool: "opencode".to_string(),
                ..Event::default()
            });
        }
    }
    (events, part_usage)
}

fn opencode_tool_events(part: &Map<String, Value>, ts: &str, model: &str) -> Vec<Event> {
    let state = part.get("state").and_then(Value::as_object);
    let status = state
        .and_then(|state| string(state.get("status")))
        .unwrap_or("");
    let call_id = string(part.get("callID"))
        .or_else(|| string(part.get("id")))
        .unwrap_or("")
        .to_string();
    let name = string(part.get("tool"))
        .or_else(|| string(part.get("name")))
        .unwrap_or("")
        .to_string();
    let input = jsonish(state.and_then(|state| state.get("input")));
    let mut call_ts = opencode_time_from_map(state.and_then(|state| state.get("time")), &["start"]);
    if call_ts.is_empty() {
        call_ts = ts.to_string();
    }

    let mut events = vec![Event {
        role: "assistant".to_string(),
        timestamp: call_ts.clone(),
        tool_calls: vec![ToolCall {
            id: call_id.clone(),
            name,
            args: input,
        }],
        model_used: model.to_string(),
        source_tool: "opencode".to_string(),
        ..Event::default()
    }];

    let mut output = jsonish(state.and_then(|state| state.get("output")));
    let mut is_error = status == "error";
    if output.is_empty() {
        output = jsonish(state.and_then(|state| state.get("error")));
        is_error = is_error || !output.is_empty();
    }
    if !output.is_empty() {
        let mut result_ts =
            opencode_time_from_map(state.and_then(|state| state.get("time")), &["end"]);
        if result_ts.is_empty() {
            result_ts = call_ts;
        }
        events.push(Event {
            role: "tool".to_string(),
            content: output,
            timestamp: result_ts,
            tool_call_id: call_id,
            is_error,
            source_tool: "opencode".to_string(),
            ..Event::default()
        });
    }
    events
}

fn opencode_part_timestamp(part: &Map<String, Value>, fallback: &str) -> String {
    let ts = opencode_time_from_map(part.get("time"), &["start", "created"]);
    if !ts.is_empty() {
        return ts;
    }
    if let Some(state) = part.get("state").and_then(Value::as_object) {
        let ts = opencode_time_from_map(state.get("time"), &["start", "created"]);
        if !ts.is_empty() {
            return ts;
        }
    }
    fallback.to_string()
}

fn opencode_record_time(doc: &Map<String, Value>) -> Option<chrono::DateTime<chrono::Utc>> {
    opencode_time_value(doc.get("time"), &["start", "created"]).or_else(|| {
        doc.get("state")
            .and_then(Value::as_object)
            .and_then(|state| opencode_time_value(state.get("time"), &["start", "created"]))
    })
}

fn opencode_time_from_map(raw: Option<&Value>, keys: &[&str]) -> String {
    opencode_time_value(raw, keys)
        .map(|ts| ts.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true))
        .unwrap_or_default()
}

fn opencode_time_value(
    raw: Option<&Value>,
    keys: &[&str],
) -> Option<chrono::DateTime<chrono::Utc>> {
    match raw? {
        Value::Object(obj) => keys
            .iter()
            .find_map(|key| opencode_parse_time(obj.get(*key))),
        other => opencode_parse_time(Some(other)),
    }
}

fn opencode_parse_time(raw: Option<&Value>) -> Option<chrono::DateTime<chrono::Utc>> {
    match raw? {
        Value::Number(number) => number.as_f64().and_then(opencode_unix_time),
        Value::String(text) => {
            let normalized = text.replace('Z', "+00:00");
            chrono::DateTime::parse_from_rfc3339(&normalized)
                .map(|ts| ts.with_timezone(&chrono::Utc))
                .ok()
                .or_else(|| {
                    chrono::NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%dT%H:%M:%S")
                        .ok()
                        .map(|ts| ts.and_utc())
                })
                .or_else(|| text.parse::<f64>().ok().and_then(opencode_unix_time))
        }
        _ => None,
    }
}

fn opencode_unix_time(value: f64) -> Option<chrono::DateTime<chrono::Utc>> {
    if value <= 0.0 {
        return None;
    }
    if value > 1e12 {
        let ms = value as i64;
        let secs = ms / 1000;
        let nsec = ((ms % 1000) * 1_000_000) as u32;
        chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nsec)
    } else {
        chrono::DateTime::<chrono::Utc>::from_timestamp(value as i64, 0)
    }
}

fn opencode_session_model(session: &Map<String, Value>) -> String {
    session
        .get("model")
        .and_then(Value::as_object)
        .and_then(|model| {
            string(model.get("id"))
                .or_else(|| string(model.get("modelID")))
                .map(str::to_string)
        })
        .filter(|model| !model.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn opencode_message_model(msg: &Map<String, Value>) -> String {
    string(msg.get("modelID"))
        .or_else(|| string(msg.get("model")))
        .unwrap_or("")
        .to_string()
}

fn add_opencode_tokens(usage: &mut BTreeMap<String, i64>, raw: Option<&Value>) -> bool {
    let Some(tokens) = raw.and_then(Value::as_object) else {
        return false;
    };
    add_usage_value(usage, "input_tokens", tokens.get("input"));
    add_usage_value(usage, "output_tokens", tokens.get("output"));
    if let Some(cache) = tokens.get("cache").and_then(Value::as_object) {
        add_usage_value(usage, "cache_read_input_tokens", cache.get("read"));
        add_usage_value(usage, "cache_creation_input_tokens", cache.get("write"));
    }
    true
}

fn add_usage(dst: &mut BTreeMap<String, i64>, src: &BTreeMap<String, i64>) {
    for (key, value) in src {
        *dst.entry(key.clone()).or_insert(0) += value;
    }
}

fn add_usage_value(usage: &mut BTreeMap<String, i64>, key: &str, raw: Option<&Value>) {
    if let Some(value) = raw.and_then(number_as_i64).filter(|value| *value > 0) {
        *usage.entry(key.to_string()).or_insert(0) += value;
    }
}

fn usage_has_values(usage: &BTreeMap<String, i64>) -> bool {
    usage.values().any(|value| *value > 0)
}

fn path_slash(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn parse_cline_value(value: &Value, model: &str) -> Option<Vec<Event>> {
    let messages = match value {
        Value::Array(items) => items.as_slice(),
        Value::Object(obj) => obj
            .get("messages")
            .or_else(|| obj.get("conversation"))
            .or_else(|| obj.get("history"))
            .and_then(Value::as_array)
            .map(Vec::as_slice)?,
        _ => return None,
    };
    let mut events = Vec::new();
    for message in messages {
        let Some(message) = message.as_object() else {
            continue;
        };
        let role = cline_role(
            string(message.get("role"))
                .or_else(|| string(message.get("speaker")))
                .or_else(|| string(message.get("author")))
                .unwrap_or(""),
        );
        let ts = cline_timestamp(
            first_value(message, &["timestamp", "ts", "createdAt", "created_at"])
                .unwrap_or(&Value::Null),
        );
        let before = events.len();
        if let Some(content) = message.get("content") {
            cline_content_events(&role, &ts, model, content, &mut events);
        }
        if events.len() == before {
            let text = string(message.get("text"))
                .or_else(|| string(message.get("message")))
                .unwrap_or("");
            if !role.is_empty() && !text.is_empty() {
                events.push(Event {
                    role,
                    content: text.to_string(),
                    timestamp: ts,
                    model_used: model.to_string(),
                    source_tool: "cline".to_string(),
                    ..Event::default()
                });
            }
        }
    }
    non_empty(events)
}

fn cline_content_events(
    role: &str,
    ts: &str,
    model: &str,
    content: &Value,
    events: &mut Vec<Event>,
) {
    match content {
        Value::String(text) if !role.is_empty() && !text.is_empty() => {
            events.push(Event {
                role: role.to_string(),
                content: text.to_string(),
                timestamp: ts.to_string(),
                model_used: model.to_string(),
                source_tool: "cline".to_string(),
                ..Event::default()
            });
        }
        Value::Array(blocks) => {
            for block in blocks {
                let Some(block) = block.as_object() else {
                    continue;
                };
                match string(block.get("type")).unwrap_or("") {
                    "text" => {
                        if let Some(text) = string(block.get("text")) {
                            if !role.is_empty() && !text.is_empty() {
                                events.push(Event {
                                    role: role.to_string(),
                                    content: text.to_string(),
                                    timestamp: ts.to_string(),
                                    model_used: model.to_string(),
                                    source_tool: "cline".to_string(),
                                    ..Event::default()
                                });
                            }
                        }
                    }
                    "thinking" => {
                        events.push(Event {
                            role: "assistant".to_string(),
                            reasoning: string(block.get("thinking"))
                                .or_else(|| string(block.get("text")))
                                .unwrap_or("")
                                .to_string(),
                            timestamp: ts.to_string(),
                            model_used: model.to_string(),
                            source_tool: "cline".to_string(),
                            ..Event::default()
                        });
                    }
                    "tool_use" => {
                        events.push(Event {
                            role: "assistant".to_string(),
                            tool_calls: vec![ToolCall {
                                id: string(block.get("id"))
                                    .or_else(|| string(block.get("tool_use_id")))
                                    .unwrap_or("")
                                    .to_string(),
                                name: string(block.get("name"))
                                    .or_else(|| string(block.get("tool_name")))
                                    .unwrap_or("")
                                    .to_string(),
                                args: jsonish(
                                    block.get("input").or_else(|| block.get("arguments")),
                                ),
                            }],
                            timestamp: ts.to_string(),
                            model_used: model.to_string(),
                            source_tool: "cline".to_string(),
                            ..Event::default()
                        });
                    }
                    "tool_result" => {
                        events.push(Event {
                            role: "tool".to_string(),
                            content: cline_tool_result_content(block),
                            timestamp: ts.to_string(),
                            tool_call_id: string(block.get("tool_use_id"))
                                .or_else(|| string(block.get("tool_call_id")))
                                .or_else(|| string(block.get("id")))
                                .unwrap_or("")
                                .to_string(),
                            is_error: block
                                .get("is_error")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            model_used: model.to_string(),
                            source_tool: "cline".to_string(),
                            ..Event::default()
                        });
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn cursor_array(value: Option<&Value>) -> Vec<Value> {
    match value {
        Some(Value::Array(items)) => items.clone(),
        Some(Value::String(raw)) if !raw.is_empty() => {
            serde_json::from_str::<Vec<Value>>(raw).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

fn cursor_object(value: Option<&Value>) -> Option<Map<String, Value>> {
    match value {
        Some(Value::Object(obj)) => Some(obj.clone()),
        Some(Value::String(raw)) if !raw.is_empty() => {
            serde_json::from_str::<Map<String, Value>>(raw).ok()
        }
        _ => None,
    }
}

fn cursor_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                item.as_object()
                    .and_then(|obj| string(obj.get("text")))
                    .map(str::to_string)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(obj) => string(obj.get("text"))
            .or_else(|| string(obj.get("content")))
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

fn cursor_role(role: &str) -> String {
    match role.to_ascii_lowercase().as_str() {
        "human" | "user" => "user".to_string(),
        "ai" | "assistant" | "bot" | "model" => "assistant".to_string(),
        "tool" => "tool".to_string(),
        other => other.to_string(),
    }
}

fn gemini_role(role: &str) -> String {
    match role {
        "model" => "assistant".to_string(),
        other => other.to_string(),
    }
}

fn cline_role(role: &str) -> String {
    match role.to_ascii_lowercase().as_str() {
        "human" => "user".to_string(),
        "ai" | "bot" => "assistant".to_string(),
        other => other.to_string(),
    }
}

fn cursor_timestamp(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.to_string(),
        Some(Value::Number(number)) => {
            let Some(ms) = number
                .as_i64()
                .or_else(|| number.as_u64().map(|n| n as i64))
            else {
                return String::new();
            };
            timestamp_millis(ms)
        }
        _ => String::new(),
    }
}

fn cline_timestamp(value: &Value) -> String {
    match value {
        Value::String(text) if text.is_empty() => String::new(),
        Value::String(text) if text.parse::<i64>().is_ok() => {
            cline_unix_timestamp(text.parse::<i64>().unwrap_or(0))
        }
        Value::String(text) => text.clone(),
        Value::Number(number) => cline_unix_timestamp(
            number
                .as_i64()
                .or_else(|| number.as_u64().map(|n| n as i64))
                .unwrap_or(0),
        ),
        _ => String::new(),
    }
}

fn cline_unix_timestamp(value: i64) -> String {
    if value > 1_000_000_000_000 {
        timestamp_millis(value)
    } else if value > 1_000_000_000 {
        chrono::DateTime::<chrono::Utc>::from_timestamp(value, 0)
            .map(|ts| ts.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_default()
    } else {
        String::new()
    }
}

fn timestamp_millis(ms: i64) -> String {
    let secs = ms / 1000;
    let nsec = ((ms % 1000) * 1_000_000) as u32;
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nsec)
        .map(|ts| ts.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_default()
}

fn timestamp_millis_nanos(ms: i64) -> String {
    let secs = ms / 1000;
    let nsec = ((ms % 1000) * 1_000_000) as u32;
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nsec)
        .map(|ts| ts.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true))
        .unwrap_or_default()
}

fn gemini_usage(value: &Value) -> Option<BTreeMap<String, i64>> {
    let obj = value.as_object()?;
    let mut usage = BTreeMap::new();
    usage.insert(
        "input_tokens".to_string(),
        first_number(
            obj,
            &[
                "promptTokenCount",
                "inputTokenCount",
                "inputTokens",
                "input_tokens",
                "prompt_tokens",
            ],
        ),
    );
    usage.insert(
        "output_tokens".to_string(),
        first_number(
            obj,
            &[
                "candidatesTokenCount",
                "outputTokenCount",
                "outputTokens",
                "output_tokens",
                "completion_tokens",
            ],
        ),
    );
    usage.insert(
        "cache_read_input_tokens".to_string(),
        first_number(
            obj,
            &[
                "cachedContentTokenCount",
                "cacheReadInputTokens",
                "cache_read_input_tokens",
            ],
        ),
    );
    Some(usage)
}

fn first_number(obj: &Map<String, Value>, keys: &[&str]) -> i64 {
    keys.iter()
        .find_map(|key| obj.get(*key))
        .and_then(number_as_i64)
        .unwrap_or(0)
}

fn sum_numbers(obj: &Map<String, Value>, keys: &[&str]) -> i64 {
    keys.iter()
        .filter_map(|key| obj.get(*key))
        .filter_map(number_as_i64)
        .sum()
}

fn number_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().map(|n| n as i64))
            .or_else(|| number.as_f64().map(|n| n as i64)),
        Value::String(text) => text.parse::<i64>().ok(),
        _ => None,
    }
}

fn boolish(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => value == "true",
        _ => false,
    }
}

fn non_empty_usage(usage: BTreeMap<String, i64>) -> Option<BTreeMap<String, i64>> {
    if usage.values().any(|value| *value > 0) {
        Some(usage)
    } else {
        None
    }
}

fn cline_tool_result_content(block: &Map<String, Value>) -> String {
    if let Some(text) = string(block.get("content")) {
        return text.to_string();
    }
    jsonish(block.get("content"))
}

fn kimi_message_events(message: &Map<String, Value>, model: &str, events: &mut Vec<Event>) {
    let role = string(message.get("role")).unwrap_or("");
    let ts = string(message.get("timestamp")).unwrap_or("").to_string();
    match message.get("content") {
        Some(Value::String(text)) => {
            if role == "tool" {
                events.push(Event {
                    role: "tool".to_string(),
                    content: text.to_string(),
                    timestamp: ts,
                    tool_call_id: string(message.get("tool_call_id"))
                        .unwrap_or("")
                        .to_string(),
                    is_error: message
                        .get("is_error")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    source_tool: "kimi_cli".to_string(),
                    ..Event::default()
                });
            } else {
                events.push(Event {
                    role: role.to_string(),
                    content: text.to_string(),
                    timestamp: ts,
                    source_tool: "kimi_cli".to_string(),
                    ..Event::default()
                });
            }
        }
        Some(Value::Array(blocks)) => {
            for block in blocks {
                let Some(block) = block.as_object() else {
                    continue;
                };
                match string(block.get("type")).unwrap_or("") {
                    "text" => events.push(Event {
                        role: role.to_string(),
                        content: string(block.get("text")).unwrap_or("").to_string(),
                        timestamp: ts.clone(),
                        source_tool: "kimi_cli".to_string(),
                        ..Event::default()
                    }),
                    "thinking" => events.push(Event {
                        role: "assistant".to_string(),
                        reasoning: string(block.get("thinking"))
                            .or_else(|| string(block.get("text")))
                            .unwrap_or("")
                            .to_string(),
                        redacted: block
                            .get("redacted")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        timestamp: ts.clone(),
                        source_tool: "kimi_cli".to_string(),
                        ..Event::default()
                    }),
                    "tool_use" => events.push(Event {
                        role: "assistant".to_string(),
                        tool_calls: vec![ToolCall {
                            id: string(block.get("id")).unwrap_or("").to_string(),
                            name: string(block.get("name"))
                                .or_else(|| {
                                    block
                                        .get("function")
                                        .and_then(Value::as_object)
                                        .and_then(|function| string(function.get("name")))
                                })
                                .unwrap_or("")
                                .to_string(),
                            args: jsonish(block.get("input").or_else(|| {
                                block.get("arguments").or_else(|| {
                                    block
                                        .get("function")
                                        .and_then(Value::as_object)
                                        .and_then(|function| function.get("arguments"))
                                })
                            })),
                        }],
                        timestamp: ts.clone(),
                        source_tool: "kimi_cli".to_string(),
                        ..Event::default()
                    }),
                    "tool_result" => events.push(Event {
                        role: "tool".to_string(),
                        timestamp: ts.clone(),
                        tool_call_id: string(block.get("tool_use_id"))
                            .or_else(|| string(block.get("tool_call_id")))
                            .unwrap_or("")
                            .to_string(),
                        content: tool_result_content(block),
                        is_error: block
                            .get("is_error")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        source_tool: "kimi_cli".to_string(),
                        ..Event::default()
                    }),
                    _ => {}
                }
            }
        }
        _ => {
            if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
                let calls = tool_calls
                    .iter()
                    .filter_map(|call| {
                        let call = call.as_object()?;
                        let function = call.get("function").and_then(Value::as_object);
                        Some(ToolCall {
                            id: string(call.get("id")).unwrap_or("").to_string(),
                            name: function
                                .and_then(|function| string(function.get("name")))
                                .unwrap_or("")
                                .to_string(),
                            args: jsonish(function.and_then(|function| function.get("arguments"))),
                        })
                    })
                    .collect::<Vec<_>>();
                if !calls.is_empty() {
                    events.push(Event {
                        role: role.to_string(),
                        tool_calls: calls,
                        timestamp: ts,
                        source_tool: "kimi_cli".to_string(),
                        ..Event::default()
                    });
                }
            }
        }
    }
    let _ = model;
}

fn jsonl_objects(raw: &str) -> Vec<Map<String, Value>> {
    raw.lines()
        .filter_map(|line| parse_jsonl_value_lenient(line.trim()))
        .filter_map(|value| match value {
            Value::Object(obj) => Some(obj),
            _ => None,
        })
        .collect()
}

fn parse_jsonl_value_lenient(line: &str) -> Option<Value> {
    serde_json::from_str::<Value>(line)
        .ok()
        .or_else(|| repair_lone_surrogates(line).and_then(|line| serde_json::from_str(&line).ok()))
}

fn repair_lone_surrogates(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut changed = false;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b'u') && i + 6 <= bytes.len() {
            let hex = &line[i + 2..i + 6];
            if let Ok(code) = u16::from_str_radix(hex, 16) {
                if (0xD800..=0xDBFF).contains(&code) {
                    if i + 12 <= bytes.len()
                        && bytes.get(i + 6) == Some(&b'\\')
                        && bytes.get(i + 7) == Some(&b'u')
                        && u16::from_str_radix(&line[i + 8..i + 12], 16)
                            .is_ok_and(|next| (0xDC00..=0xDFFF).contains(&next))
                    {
                        out.push_str(&line[i..i + 12]);
                        i += 12;
                    } else {
                        out.push_str("\\ufffd");
                        changed = true;
                        i += 6;
                    }
                    continue;
                }
                if (0xDC00..=0xDFFF).contains(&code) {
                    out.push_str("\\ufffd");
                    changed = true;
                    i += 6;
                    continue;
                }
            }
        }
        let ch = line[i..].chars().next()?;
        out.push(ch);
        i += ch.len_utf8();
    }
    changed.then_some(out)
}

fn usage_from_value(value: &Value) -> Option<BTreeMap<String, i64>> {
    let obj = value.as_object()?;
    let mut usage = BTreeMap::new();
    for (target, keys) in [
        (
            "input_tokens",
            &[
                "input_tokens",
                "prompt_tokens",
                "inputTokens",
                "promptTokenCount",
            ][..],
        ),
        (
            "output_tokens",
            &[
                "output_tokens",
                "completion_tokens",
                "outputTokens",
                "candidatesTokenCount",
            ][..],
        ),
        (
            "cache_creation_input_tokens",
            &[
                "cache_creation_input_tokens",
                "cacheCreationInputTokens",
                "cache_creation",
                "cacheWriteTokens",
            ][..],
        ),
        (
            "cache_read_input_tokens",
            &[
                "cache_read_input_tokens",
                "cacheReadInputTokens",
                "cache_read",
                "cacheReadTokens",
            ][..],
        ),
    ] {
        if let Some(value) = keys
            .iter()
            .find_map(|key| obj.get(*key))
            .and_then(number_as_i64)
        {
            usage.insert(target.to_string(), value);
        }
    }
    if usage.is_empty() {
        None
    } else {
        Some(usage)
    }
}

fn tool_result_content(block: &Map<String, Value>) -> String {
    if let Some(text) = string(block.get("content")) {
        return text.to_string();
    }
    jsonish(block.get("content").or_else(|| block.get("response")))
}

fn copilot_usage(span: &Map<String, Value>) -> BTreeMap<String, i64> {
    let mut usage = BTreeMap::new();
    for (target, key) in [
        ("input_tokens", "gen_ai.usage.input_tokens"),
        ("output_tokens", "gen_ai.usage.output_tokens"),
        (
            "cache_creation_input_tokens",
            "gen_ai.usage.cache_creation_input_tokens",
        ),
        (
            "cache_read_input_tokens",
            "gen_ai.usage.cache_read_input_tokens",
        ),
    ] {
        if let Some(value) = copilot_i64_attr(span, key).filter(|value| *value > 0) {
            usage.insert(target.to_string(), value);
        }
    }
    usage
}

fn copilot_span_content(span: &Map<String, Value>) -> String {
    for key in ["content", "text", "message", "body"] {
        if let Some(text) = string(span.get(key)).filter(|value| !value.is_empty()) {
            return text.to_string();
        }
    }
    for key in ["content", "message", "body", "gen_ai.response.text"] {
        if let Some(text) = copilot_string_attr(span, key).filter(|value| !value.is_empty()) {
            return text;
        }
    }
    String::new()
}

fn copilot_attr<'a>(span: &'a Map<String, Value>, target: &str) -> Option<&'a Value> {
    match span.get("attributes")? {
        Value::Object(attrs) => attrs.get(target),
        Value::Array(attrs) => attrs.iter().find_map(|attr| {
            let attr = attr.as_object()?;
            if string(attr.get("key")) == Some(target) {
                attr.get("value")
            } else {
                None
            }
        }),
        _ => None,
    }
}

fn copilot_string_attr(span: &Map<String, Value>, target: &str) -> Option<String> {
    match copilot_attr(span, target)? {
        Value::String(text) => Some(text.clone()),
        Value::Object(obj) => string(obj.get("stringValue")).map(str::to_string),
        _ => None,
    }
}

fn copilot_i64_attr(span: &Map<String, Value>, target: &str) -> Option<i64> {
    let value = copilot_attr(span, target)?;
    number_as_i64(value).or_else(|| {
        value
            .as_object()
            .and_then(|obj| {
                obj.get("intValue")
                    .or_else(|| obj.get("stringValue"))
                    .or_else(|| obj.get("doubleValue"))
            })
            .and_then(number_as_i64)
    })
}

fn copilot_bool_attr(span: &Map<String, Value>, target: &str) -> bool {
    let Some(value) = copilot_attr(span, target) else {
        return false;
    };
    value.as_bool().unwrap_or_else(|| {
        value
            .as_object()
            .and_then(|obj| obj.get("boolValue").and_then(Value::as_bool))
            .unwrap_or(false)
    })
}

fn copilot_timestamp(raw: Option<&Value>) -> String {
    let Some(value) = raw.and_then(number_as_i64) else {
        return String::new();
    };
    let secs = value / 1_000_000_000;
    let nsec = (value % 1_000_000_000) as u32;
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nsec)
        .map(|ts| ts.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true))
        .unwrap_or_default()
}

fn first_value<'a>(obj: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| obj.get(*key))
}

fn string(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str)
}

fn jsonish(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(other) => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn non_empty(events: Vec<Event>) -> Option<Vec<Event>> {
    if events.is_empty() {
        None
    } else {
        Some(events)
    }
}

fn is_cline_task_file(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("api_conversation_history.json" | "ui_messages.json" | "task_metadata.json")
    )
}

fn read_json_value(path: &Path) -> Option<Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn cline_model(metadata: &Map<String, Value>) -> String {
    for key in ["model", "modelId", "model_id", "apiModelId"] {
        if let Some(model) = string(metadata.get(key)) {
            if !model.is_empty() {
                return model.to_string();
            }
        }
    }
    if let Some(config) = metadata.get("apiConfiguration").and_then(Value::as_object) {
        for key in ["model", "modelId", "model_id", "apiModelId"] {
            if let Some(model) = string(config.get(key)) {
                if !model.is_empty() {
                    return model.to_string();
                }
            }
        }
    }
    "unknown".to_string()
}

fn parse_cline_ui_messages(value: &Value, model: &str) -> Vec<Event> {
    let messages: &[Value] = match value {
        Value::Array(items) => items,
        Value::Object(obj) => obj
            .get("messages")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        _ => &[],
    };
    let mut events = Vec::new();
    for message in messages {
        let Some(message) = message.as_object() else {
            continue;
        };
        let text = string(message.get("text"))
            .or_else(|| string(message.get("content")))
            .or_else(|| string(message.get("message")))
            .unwrap_or("");
        if text.is_empty() {
            continue;
        }
        let ts = cline_timestamp(
            first_value(message, &["ts", "timestamp", "createdAt", "created_at"])
                .unwrap_or(&Value::Null),
        );
        let kind = string(message.get("type")).unwrap_or("");
        let ask = string(message.get("ask")).unwrap_or("");
        let say = string(message.get("say")).unwrap_or("");
        let mut role = "assistant";
        if kind == "ask" || !ask.is_empty() {
            role = "user";
        }
        if say == "tool" || ask == "tool" {
            role = "assistant";
        }
        events.push(Event {
            role: role.to_string(),
            content: text.to_string(),
            timestamp: ts,
            model_used: model.to_string(),
            source_tool: "cline".to_string(),
            ..Event::default()
        });
    }
    events
}

fn append_cline_event(events: &mut Vec<Event>, seen: &mut BTreeSet<String>, mut event: Event) {
    if event.source_tool.is_empty() {
        event.source_tool = "cline".to_string();
    }
    if event.model_used.is_empty() {
        event.model_used = "unknown".to_string();
    }
    if event.role.is_empty() {
        return;
    }
    let key = cline_event_key(&event);
    if seen.insert(key) {
        events.push(event);
    }
}

fn cline_event_key(event: &Event) -> String {
    let tool_parts = event
        .tool_calls
        .iter()
        .map(|tool| format!("{}:{}:{}", tool.id, tool.name, tool.args))
        .collect::<Vec<_>>()
        .join(",");
    [
        event.role.as_str(),
        event.content.as_str(),
        event.timestamp.as_str(),
        event.tool_call_id.as_str(),
        tool_parts.as_str(),
    ]
    .join("\0")
}

fn apply_cline_metadata_timestamps(events: &mut [Event], metadata: &Map<String, Value>) {
    if events.is_empty() || events.iter().any(|event| !event.timestamp.is_empty()) {
        return;
    }
    let start = cline_timestamp(
        first_value(metadata, &["createdAt", "created_at", "ts"]).unwrap_or(&Value::Null),
    );
    let end = cline_timestamp(
        first_value(metadata, &["updatedAt", "updated_at", "lastUpdatedAt"])
            .unwrap_or(&Value::Null),
    );
    if !start.is_empty() {
        events[0].timestamp = start;
    }
    if !end.is_empty() {
        if let Some(last) = events.last_mut() {
            last.timestamp = end;
        }
    }
}

fn session_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("session")
        .to_string()
}
