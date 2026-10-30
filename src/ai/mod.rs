use serde_json::{json, Value as Json};
use std::time::Instant;
use tokio::sync::mpsc;

use crate::db;
use crate::tools;
use crate::util::local_now;
use sqlx::PgPool;
use uuid::Uuid;

const MAX_TOOL_STEPS: usize = 8;
const HISTORY_LIMIT: i64 = 30;

#[derive(Clone)]
pub struct AiClient {
    http: reqwest::Client,
    base_url: String,
    model: String,
    max_tokens: u32,
}

impl AiClient {
    pub fn new(base_url: &str, model: &str, max_tokens: u32) -> Self {
        Self {
            http: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(15))
                .read_timeout(std::time::Duration::from_secs(300))
                .user_agent("halisai/0.1")
                .build()
                .expect("tao http client"),
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            max_tokens,
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }
}

/// Su kien phat ra cho trinh duyet.
#[derive(Clone, Debug)]
pub enum AgentEvent {
    Thinking(String),
    Delta(String),
    ToolStart { name: String },
    ToolDone { name: String, ms: u64, summary: String },
    Done { truncated: bool, duration_ms: u64 },
    Error(String),
}

/// Ket qua mot luot tra loi hoan chinh (mot "turn" gom nhieu "step").
#[derive(Debug, Default)]
pub struct TurnOutcome {
    pub content: String,
    pub thinking: String,
    pub finish_reason: String,
    pub tools_used: Vec<Json>,
    pub cancelled: bool,
}

#[derive(Debug, Default)]
struct StepResult {
    content: String,
    thinking: String,
    tool_calls: Vec<ToolCall>,
    finish_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct ToolCall {
    id: String,
    name: String,
    arguments: String,
}

fn parse_sse_payload(data: &str) -> Option<Json> {
    if data.trim() == "[DONE]" {
        return None;
    }
    serde_json::from_str::<Json>(data).ok()
}

/// Trich text tu mot SSE chunk.
fn extract_step(chunk: &Json) -> StepResult {
    let mut out = StepResult::default();
    let Some(choices) = chunk.get("choices").and_then(|c| c.as_array()) else {
        return out;
    };
    for choice in choices {
        if let Some(fr) = choice.get("finish_reason").and_then(|f| f.as_str()) {
            out.finish_reason = Some(fr.to_string());
        }
        let Some(delta) = choice.get("delta") else { continue };
        // Noi dung chinh
        if let Some(c) = delta.get("content").and_then(|c| c.as_str()) {
            out.content.push_str(c);
        }
        // Suy nghi: ho tro reasoning hoac reasoning_content
        for key in ["reasoning", "reasoning_content"] {
            if let Some(r) = delta.get(key).and_then(|r| r.as_str()) {
                out.thinking.push_str(r);
            }
        }
        // Tool calls tach theo index
        if let Some(tcs) = delta.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in tcs {
                let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                while out.tool_calls.len() <= index {
                    out.tool_calls.push(ToolCall {
                        id: String::new(),
                        name: String::new(),
                        arguments: String::new(),
                    });
                }
                let slot = &mut out.tool_calls[index];
                if let Some(id) = tc.get("id").and_then(|i| i.as_str()) {
                    if !id.is_empty() {
                        slot.id = id.to_string();
                    }
                }
                if let Some(f) = tc.get("function") {
                    if let Some(n) = f.get("name").and_then(|n| n.as_str()) {
                        slot.name.push_str(n);
                    }
                    if let Some(a) = f.get("arguments").and_then(|a| a.as_str()) {
                        slot.arguments.push_str(a);
                    }
                }
            }
        }
    }
    out
}

/// Gop cac fragment tool_call tu cac chunk SSE khac nhau (theo index).
fn merge_tool_calls(acc: &mut Vec<ToolCall>, incoming: &[ToolCall]) {
    for (idx, tc) in incoming.iter().enumerate() {
        while acc.len() <= idx {
            acc.push(ToolCall {
                id: String::new(),
                name: String::new(),
                arguments: String::new(),
            });
        }
        let slot = &mut acc[idx];
        if !tc.id.is_empty() {
            slot.id = tc.id.clone();
        }
        slot.name.push_str(&tc.name);
        slot.arguments.push_str(&tc.arguments);
    }
}

/// Doc stream SSE tu gateway, forward tung phan ra client qua channel.
/// Tra ve (ket qua step, client con ket noi hay khong).
async fn stream_step(
    ai: &AiClient,
    body: Json,
    tx: &mpsc::Sender<AgentEvent>,
) -> Result<(StepResult, bool), String> {
    let resp = ai
        .http
        .post(ai.endpoint())
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("loi goi gateway: {e}");
            "gateway".to_string()
        })?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        // Chi log phia server, khong hien thi cho nguoi dung
        tracing::error!("gateway tra ve {status}: {}", &text[..text.len().min(400)]);
        return Err("gateway".into());
    }

    let mut buffer = Vec::new();
    let mut stream = resp.bytes_stream();
    use futures::StreamExt;
    let mut step = StepResult::default();
    let mut seen_done = false;
    let mut client_gone = false;

    'outer: while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| {
            tracing::error!("loi doc stream: {e}");
            "stream".to_string()
        })?;
        buffer.extend_from_slice(&bytes);
        while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buffer.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\n', '\r']);
            let Some(data) = line.strip_prefix("data:") else { continue };
            let data = data.trim_start();
            if data.is_empty() {
                continue;
            }
            if data == "[DONE]" {
                seen_done = true;
                break 'outer;
            }
            let Some(chunk_json) = parse_sse_payload(data) else { continue };
            let part = extract_step(&chunk_json);
            if !part.content.is_empty() {
                if tx.send(AgentEvent::Delta(part.content.clone())).await.is_err() {
                    client_gone = true;
                    break 'outer;
                }
                step.content.push_str(&part.content);
            }
            if !part.thinking.is_empty() {
                if tx.send(AgentEvent::Thinking(part.thinking.clone())).await.is_err() {
                    client_gone = true;
                    break 'outer;
                }
                step.thinking.push_str(&part.thinking);
            }
            if !part.tool_calls.is_empty() {
                merge_tool_calls(&mut step.tool_calls, &part.tool_calls);
            }
            if part.finish_reason.is_some() {
                step.finish_reason = part.finish_reason.clone();
            }
        }
    }
    let _ = seen_done;
    Ok((step, client_gone))
}

/// Chay mot luot (turn): vong lap model -> tool -> model cho den khi khong con tool.
/// Ket qua duoc luu vao DB va phat su kien qua channel.
pub async fn run_turn(
    ai: &AiClient,
    pool: &PgPool,
    conversation_id: Uuid,
    system_prompt: &str,
    tx: mpsc::Sender<AgentEvent>,
) -> Result<TurnOutcome, String> {
    // Lich su tin nhan
    let history = db::chat::list_messages(pool, conversation_id, HISTORY_LIMIT)
        .await
        .map_err(|e| {
            tracing::error!("doc lich su that bai: {e}");
            "db".to_string()
        })?;
    let mut messages: Vec<Json> = Vec::with_capacity(history.len() + 2);
    let full_system = format!("{system_prompt}\n\nThời gian hiện tại của người dùng: {}.", crate::util::format_full(local_now()));
    messages.push(json!({ "role": "system", "content": full_system }));
    for m in &history {
        if m.content.trim().is_empty() {
            continue;
        }
        messages.push(json!({ "role": m.role, "content": m.content }));
    }

    let tool_schemas = tools::schemas();
    let mut outcome = TurnOutcome::default();
    let mut cancelled = false;

    for _step in 0..MAX_TOOL_STEPS {
        let mut body = json!({
            "model": ai.model,
            "messages": messages,
            "stream": true,
            "max_tokens": ai.max_tokens,
        });
        if _step == 0 {
            body["tools"] = tool_schemas.clone();
        }

        let (step, client_gone) = stream_step(ai, body, &tx).await?;
        if client_gone {
            cancelled = true;
        }

        outcome.content.push_str(&step.content);
        outcome.thinking.push_str(&step.thinking);

        if step.tool_calls.is_empty() {
            outcome.finish_reason = step.finish_reason.unwrap_or_else(|| "stop".into());
            break;
        }

        // Co tool call: thuc thi roi tiep tuc vong lap
        let assistant_msg = json!({
            "role": "assistant",
            "content": if step.content.is_empty() { Json::Null } else { json!(step.content) },
            "tool_calls": step.tool_calls.iter().map(|tc| json!({
                "id": tc.id,
                "type": "function",
                "function": { "name": tc.name, "arguments": tc.arguments }
            })).collect::<Vec<_>>(),
        });
        messages.push(assistant_msg);

        for tc in &step.tool_calls {
            if tx.send(AgentEvent::ToolStart { name: tc.name.clone() }).await.is_err() {
                cancelled = true;
                break;
            }
            let t0 = Instant::now();
            let args: Json = serde_json::from_str(&tc.arguments).unwrap_or(json!({}));
            let result = tools::execute(&tc.name, &args).await;
            let ms = t0.elapsed().as_millis() as u64;
            if tx
                .send(AgentEvent::ToolDone {
                    name: tc.name.clone(),
                    ms,
                    summary: result.summary.clone(),
                })
                .await
                .is_err()
            {
                cancelled = true;
            }
            outcome.tools_used.push(json!({
                "name": tc.name,
                "summary": result.summary,
                "ok": result.ok,
                "ms": ms,
            }));
            let content = if result.ok {
                result.output
            } else {
                format!("LỖI: {}", result.output)
            };
            messages.push(json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": content,
            }));
        }

        if cancelled {
            break;
        }
    }

    if outcome.finish_reason.is_empty() {
        outcome.finish_reason = if cancelled { "stopped".into() } else { "stop".into() };
    }
    outcome.cancelled = cancelled;
    Ok(outcome)
}

/// Luu ket qua turn vao db.
pub async fn save_turn(
    pool: &PgPool,
    conversation_id: Uuid,
    outcome: &TurnOutcome,
    duration_ms: u64,
) -> Option<uuid::Uuid> {
    let tools_json = if outcome.tools_used.is_empty() {
        None
    } else {
        Some(Json::Array(outcome.tools_used.clone()))
    };
    let msg = db::chat::insert_message(
        pool,
        conversation_id,
        db::chat::NewMessage {
            role: "assistant",
            content: &outcome.content,
            thinking: if outcome.thinking.is_empty() { None } else { Some(&outcome.thinking) },
            tools: tools_json.as_ref(),
            finish_reason: Some(&outcome.finish_reason),
            duration_ms: Some(duration_ms as i32),
        },
    )
    .await
    .ok()?;
    let _ = db::chat::touch_conversation(pool, conversation_id).await;
    Some(msg.id)
}

pub fn generic_error_message() -> String {
    "Halis tạm thời không phản hồi. Vui lòng thử lại sau ít phút.".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_content() {
        let chunk = json!({
            "choices": [{"index": 0, "delta": {"content": "xin", "role": "assistant"}, "finish_reason": null}]
        });
        let s = extract_step(&chunk);
        assert_eq!(s.content, "xin");
        assert!(s.tool_calls.is_empty());
    }

    #[test]
    fn test_extract_reasoning_and_tools() {
        let chunk = json!({
            "choices": [{"index": 0, "delta": {
                "content": null,
                "reasoning": " suy nghi...",
                "tool_calls": [
                    {"index": 0, "id": "call_1", "function": {"name": "current_time", "arguments": "{}"}}
                ]
            }, "finish_reason": "tool_calls"}]
        });
        let s = extract_step(&chunk);
        assert_eq!(s.thinking, " suy nghi...");
        assert_eq!(s.tool_calls.len(), 1);
        assert_eq!(s.tool_calls[0].name, "current_time");
        assert_eq!(s.finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn test_tool_args_accumulate() {
        let c1 = json!({"choices": [{"delta": {"tool_calls": [
            {"index": 0, "id": "a", "function": {"name": "calc", "arguments": "{\"expr"}}
        ]}}]});
        let c2 = json!({"choices": [{"delta": {"tool_calls": [
            {"index": 0, "function": {"arguments": "\": 42}"}}
        ]}}]});
        let s1 = extract_step(&c1);
        let s2 = extract_step(&c2);
        let mut acc = s1.tool_calls;
        merge_tool_calls(&mut acc, &s2.tool_calls);
        assert_eq!(acc.len(), 1);
        assert_eq!(acc[0].id, "a");
        assert_eq!(acc[0].name, "calc");
        assert_eq!(acc[0].arguments, "{\"expr\": 42}");
    }
}
