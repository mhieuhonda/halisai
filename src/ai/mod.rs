use serde_json::{json, Value as Json};
use std::time::Instant;
use tokio::sync::mpsc;

use crate::db;
use crate::tools::Tools;
use crate::usage;
use sqlx::PgPool;
use uuid::Uuid;

const MAX_TOOL_STEPS: usize = 10;

/// Một số model phát lệnh gọi tool ngay trong nội dung văn bản thay vì
/// qua cơ chế tool_calls chuẩn. Bộ lọc này che khối lệnh khỏi luồng hiển thị
/// và trả lại để thực thi như một tool call thật.
///
/// Dạng được hỗ trợ:
/// <function=tên>
/// <parameter=khóa>giá trị</parameter>
/// </function>
pub struct DeltaFilter {
    buf: String,
    /// Vị trí đã quét tới trong buf (đã emit hoặc đã tách thành khối lệnh).
    pos: usize,
    blocks: Vec<ToolCall>,
    counter: usize,
}

const FN_OPEN: &str = "<function=";
const FN_CLOSE: &str = "</function>";

impl DeltaFilter {
    pub fn new() -> Self {
        Self { buf: String::new(), pos: 0, blocks: Vec::new(), counter: 0 }
    }

    /// Nạp thêm văn bản, trả về phần được phép hiển thị ngay.
    pub fn push(&mut self, s: &str) -> String {
        self.buf.push_str(s);
        self.drain(false)
    }

    /// Kết thúc một step: trả về phần văn bản còn lại (không giữ lại gì).
    pub fn finish(&mut self) -> String {
        let rest = self.drain(true);
        self.buf.clear();
        self.pos = 0;
        rest
    }

    /// Các khối lệnh đã tách được từ văn bản.
    pub fn take_blocks(&mut self) -> Vec<ToolCall> {
        std::mem::take(&mut self.blocks)
    }

    /// true nếu buf từ pos bắt đầu bằng (hoặc là tiền tố của) FN_OPEN.
    fn looks_like_open(&self) -> bool {
        let tail = &self.buf[self.pos..];
        if tail.len() >= FN_OPEN.len() {
            tail.starts_with(FN_OPEN)
        } else {
            FN_OPEN.starts_with(tail)
        }
    }

    fn drain(&mut self, final_flush: bool) -> String {
        let mut out = String::new();
        loop {
            let rest = &self.buf[self.pos..];
            if rest.is_empty() {
                break;
            }
            let Some(rel) = rest.find('<') else {
                // Không còn '<': với flush cuối emit hết, ngược lại giữ 1 ký tự
                // phòng trường hợp nó là mảnh của "<function=" vừa tới.
                out.push_str(rest);
                self.pos = self.buf.len();
                break;
            };
            // Văn bản trước '<' luôn được emit
            out.push_str(&rest[..rel]);
            self.pos += rel;
            if !self.looks_like_open() {
                // '<' thường: emit luôn và quét tiếp
                out.push('<');
                self.pos += 1;
                continue;
            }
            // Có khả năng là khối lệnh
            let tail = &self.buf[self.pos..];
            if tail.len() < FN_OPEN.len() {
                // Chưa đủ độ dài: giữ lại nếu chưa flush cuối
                if final_flush {
                    // Không phải khối (thiếu ">") — emit như văn bản
                    out.push_str(tail);
                    self.pos = self.buf.len();
                }
                break;
            }
            // Đủ "<function=" — tìm tên và thẻ đóng
            let after_name = match tail[FN_OPEN.len()..].find('>') {
                Some(g) => &tail[FN_OPEN.len()..FN_OPEN.len() + g],
                None => {
                    if final_flush {
                        out.push_str(tail);
                        self.pos = self.buf.len();
                    }
                    break;
                }
            };
            if after_name.is_empty() || !after_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                // Không phải tên hàm hợp lệ — emit '<' và bỏ qua
                out.push('<');
                self.pos += 1;
                continue;
            }
            let name = after_name.to_string();
            match tail.find(FN_CLOSE) {
                Some(end_rel) => {
                    let body = tail[..end_rel].to_string();
                    if let Some(call) = parse_block(&name, &body, &mut self.counter) {
                        self.blocks.push(call);
                    }
                    self.pos += end_rel + FN_CLOSE.len();
                }
                None => {
                    if final_flush {
                        // Khối chưa đóng: vẫn cố parse phần có được
                        if let Some(call) = parse_block(&name, tail, &mut self.counter) {
                            self.blocks.push(call);
                        }
                        self.pos = self.buf.len();
                    }
                    // Chưa có thẻ đóng: giữ nguyên chờ chunk sau
                    break;
                }
            }
        }
        out
    }
}

/// Tách các cặp <parameter=khóa>giá trị</parameter> thành tham số JSON.
fn parse_block(name: &str, body: &str, counter: &mut usize) -> Option<ToolCall> {
    let mut args = serde_json::Map::new();
    let mut rest = body;
    while let Some(p) = rest.find("<parameter=") {
        let after = &rest[p + 11..];
        let Some(g) = after.find('>') else { break };
        let key = &after[..g];
        let after_key = &after[g + 1..];
        let Some(close) = after_key.find("</parameter>") else { break };
        let raw = &after_key[..close];
        let value = raw.trim();
        // Thử đọc như JSON, không được thì coi là chuỗi
        let val = match serde_json::from_str::<Json>(value) {
            Ok(v) => v,
            Err(_) => Json::String(value.to_string()),
        };
        if !key.is_empty() {
            args.insert(key.to_string(), val);
        }
        rest = &after_key[close + 12..];
    }
    let id = format!("textcall_{}", counter);
    *counter += 1;
    Some(ToolCall {
        id,
        name: name.to_string(),
        arguments: Json::Object(args).to_string(),
    })
}

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
    Usage(usage::Snapshot),
    Done { truncated: bool, duration_ms: u64 },
    Error(String),
}

/// Loi trong mot luot chay agent.
#[derive(Debug)]
pub enum TurnError {
    /// Gateway tra 429: het han muc, can ep thanh usage ve 100%.
    RateLimited,
    /// Loi gateway / stream khac (da log chi tiet).
    Gateway,
    Db,
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
pub(crate) struct ToolCall {
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
) -> Result<(StepResult, bool), TurnError> {
    let resp = ai.http.post(ai.endpoint()).json(&body).send().await.map_err(|e| {
        tracing::error!("loi goi gateway: {e}");
        TurnError::Gateway
    })?;
    if !resp.status().is_success() {
        let status = resp.status();
        if status.as_u16() == 429 {
            tracing::warn!("gateway tra ve 429");
            return Err(TurnError::RateLimited);
        }
        let text = resp.text().await.unwrap_or_default();
        // Chi log phia server, khong hien thi cho nguoi dung
        tracing::error!("gateway tra ve {status}: {}", &text[..text.len().min(400)]);
        return Err(TurnError::Gateway);
    }

    let mut buffer = Vec::new();
    let mut stream = resp.bytes_stream();
    use futures::StreamExt;
    let mut step = StepResult::default();
    let mut seen_done = false;
    let mut client_gone = false;
    let mut filter = DeltaFilter::new();

    'outer: while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| {
            tracing::error!("loi doc stream: {e}");
            TurnError::Gateway
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
                // Che khối lệnh tool dạng văn bản, chỉ phát phần hiển thị được
                let visible = filter.push(&part.content);
                if !visible.is_empty() {
                    if tx.send(AgentEvent::Delta(visible.clone())).await.is_err() {
                        client_gone = true;
                        break 'outer;
                    }
                    step.content.push_str(&visible);
                }
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
    // Phần văn bản còn kẹt trong bộ lọc và các lệnh gọi tool dạng văn bản
    let tail = filter.finish();
    if !tail.is_empty() && !client_gone && tx.send(AgentEvent::Delta(tail.clone())).await.is_ok() {
        step.content.push_str(&tail);
    }
    let text_blocks = filter.take_blocks();
    if !text_blocks.is_empty() {
        tracing::debug!("tách {} lệnh gọi tool từ văn bản", text_blocks.len());
        step.tool_calls.extend(text_blocks);
    }
    Ok((step, client_gone))
}

/// Chay mot luot (turn): vong lap model -> tool -> model cho den khi khong con tool.
/// Ket qua duoc luu vao DB va phat su kien qua channel.
pub async fn run_turn(
    ai: &AiClient,
    tools: &Tools,
    pool: &PgPool,
    conversation_id: Uuid,
    system_prompt: &str,
    lang: &str,
    tx: mpsc::Sender<AgentEvent>,
) -> Result<TurnOutcome, TurnError> {
    // Lich su tin nhan
    let history = db::chat::list_messages(pool, conversation_id, HISTORY_LIMIT)
        .await
        .map_err(|e| {
            tracing::error!("doc lich su that bai: {e}");
            TurnError::Db
        })?;
    let mut messages: Vec<Json> = Vec::with_capacity(history.len() + 2);
    let full_system = build_system_prompt(system_prompt, lang);
    messages.push(json!({ "role": "system", "content": full_system }));
    for m in &history {
        if m.content.trim().is_empty() {
            continue;
        }
        messages.push(json!({ "role": m.role, "content": m.content }));
    }

    let tool_schemas = crate::tools::schemas();
    let mut outcome = TurnOutcome::default();
    let mut cancelled = false;

    for _step in 0..MAX_TOOL_STEPS {
        let mut body = json!({
            "model": ai.model,
            "messages": messages,
            "stream": true,
            "max_tokens": ai.max_tokens,
        });
        body["tools"] = tool_schemas.clone();

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
            "content": step.content.clone(),
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
            let result = tools.execute(&tc.name, &args, lang).await;
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
                format!("ERROR: {}", result.output)
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

pub fn generic_error_message(lang: &str) -> String {
    if lang == "en" {
        "Halis is temporarily unavailable. Please try again in a few minutes.".into()
    } else {
        "Halis tạm thời không phản hồi. Vui lòng thử lại sau ít phút.".into()
    }
}

/// Ghép system prompt với ngữ cảnh động: ngôn ngữ người dùng và thời gian hiện tại.
fn build_system_prompt(base: &str, lang: &str) -> String {
    let now = crate::util::local_now();
    let lang_name = if lang == "en" { "English" } else { "Vietnamese" };
    let date = now.format(time::macros::format_description!(
        "[day]/[month]/[year] [hour]:[minute]:[second]"
    )).unwrap_or_default();
    format!(
        "{base}\n\n[Runtime context]\n- The user's interface language is {lang_name}. Reply in the language the user writes in; default to {lang_name}.\n- Current local time for the user: {date} (GMT+7)."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_filter_hides_tool_block() {
        let mut f = DeltaFilter::new();
        let out = f.push("Đây là câu trả lời. <function=web_search>\n<parameter=queries>");
        assert_eq!(out, "Đây là câu trả lời. ");
        let out = f.push("[\"tin tức AI\"]</parameter>\n</function>\nVà tiếp tục bình thường.");
        assert_eq!(out, "\nVà tiếp tục bình thường.");
        let blocks = f.take_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "web_search");
        assert!(blocks[0].arguments.contains("tin tức AI"));
    }

    #[test]
    fn test_delta_filter_partial_chunks() {
        let mut f = DeltaFilter::new();
        // Chunk cắt giữa "<function" — không được emit mảnh này
        let a = f.push("trước <fun");
        assert_eq!(a, "trước ");
        let b = f.push("ction=datetime>\n<parameter=timezone>America/New_York</parameter>\n</function>");
        assert_eq!(b, "");
        let c = f.push(" sau");
        assert_eq!(c, " sau");
        let blocks = f.take_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "datetime");
        assert!(blocks[0].arguments.contains("America/New_York"));
    }

    #[test]
    fn test_delta_filter_keeps_normal_angle_brackets() {
        let mut f = DeltaFilter::new();
        let out = f.push("So sánh a < b và 3 > 2 nhé.");
        assert_eq!(out, "So sánh a < b và 3 > 2 nhé.");
        assert!(f.take_blocks().is_empty());
    }

    #[test]
    fn test_delta_filter_unterminated_block() {
        let mut f = DeltaFilter::new();
        f.push("gọi <function=web_fetch>\n<parameter=url>https://example.com</parameter>\n</fun");
        let out = f.finish();
        assert_eq!(out, "");
        let blocks = f.take_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "web_fetch");
        assert!(blocks[0].arguments.contains("example.com"));
    }

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
