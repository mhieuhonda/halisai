use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Response};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::ai::{self, AgentEvent};
use crate::auth::AuthUser;
use crate::db;
use crate::error::{error_page, AppResult};
use crate::routes::{build_ctx, group_conversations, ConvItem, ConversationsPartial, Ctx};
use crate::state::AppState;
use crate::usage;
use crate::util::{markdown_to_html, truncate_chars};
use time::OffsetDateTime;

#[derive(Template)]
#[template(path = "chat.html")]
pub struct ChatPage {
    pub ctx: Ctx,
    pub t: &'static crate::i18n::Strings,
    pub active: Option<ActiveConv>,
    pub messages: Vec<MsgView>,
}

#[derive(Clone, Debug)]
pub struct ActiveConv {
    pub id: Uuid,
    pub title: String,
}

#[derive(Clone, Debug)]
pub struct MsgView {
    pub role: String,
    pub html: String,
    pub thinking: Option<String>,
    pub tools: Vec<ToolView>,
    pub truncated: bool,
    pub rel: String,
    pub dur_s: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct ToolView {
    pub name: String,
    pub summary: String,
    pub ms: i64,
}

fn to_msg_views(msgs: &[db::chat::Message], now: OffsetDateTime, lang: &str) -> Vec<MsgView> {
    msgs.iter()
        .map(|m| MsgView {
            role: m.role.clone(),
            html: if m.role == "user" {
                crate::util::escape_html(&m.content)
            } else {
                markdown_to_html(&m.content)
            },
            thinking: m.thinking.clone().filter(|t| !t.trim().is_empty()),
            tools: parse_tools(m.tools.as_ref()),
            truncated: m.finish_reason.as_deref() == Some("length"),
            rel: crate::i18n::relative_time(m.created_at, now, lang),
            dur_s: m.duration_ms.map(|d| (d as f64) / 1000.0),
        })
        .collect()
}

fn parse_tools(tools: Option<&serde_json::Value>) -> Vec<ToolView> {
    tools
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    Some(ToolView {
                        name: t.get("name")?.as_str()?.to_string(),
                        summary: t.get("summary")?.as_str()?.to_string(),
                        ms: t.get("ms")?.as_u64()? as i64,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub async fn chat_home(State(state): State<AppState>, auth: AuthUser) -> AppResult<Response> {
    let ctx = build_ctx(&state, &auth).await;
    let t = crate::i18n::Strings::for_lang(&auth.user.lang);
    Ok(Html(ChatPage { ctx, t, active: None, messages: vec![] }.render()?).into_response())
}

pub async fn chat_page(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    let Some(conv) = db::chat::get_conversation(&state.pool, id, auth.user.id).await? else {
        return Err(crate::error::AppError::NotFound(
            crate::i18n::Strings::for_lang(&auth.user.lang).conv_not_found.to_string(),
        ));
    };
    let msgs = db::chat::list_messages(&state.pool, id, 100).await?;
    let ctx = build_ctx(&state, &auth).await;
    let t = crate::i18n::Strings::for_lang(&auth.user.lang);
    let now = OffsetDateTime::now_utc();
    Ok(Html(
        ChatPage {
            ctx,
            t,
            active: Some(ActiveConv { id: conv.id, title: conv.title }),
            messages: to_msg_views(&msgs, now, &auth.user.lang),
        }
        .render()?,
    )
    .into_response())
}

#[derive(Deserialize)]
pub struct ConversationsQuery {
    active: Option<Uuid>,
}

pub async fn conversations_partial(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ConversationsQuery>,
) -> AppResult<Response> {
    let convs = db::chat::list_conversations(&state.pool, auth.user.id).await?;
    let lang = auth.user.lang.clone();
    let items = convs
        .into_iter()
        .map(|c| ConvItem {
            id: c.id,
            title: c.title,
            active: q.active == Some(c.id),
            updated_at: c.updated_at,
        })
        .collect();
    let groups = group_conversations(items, &lang);
    Ok(Html(ConversationsPartial { groups, t: crate::i18n::Strings::for_lang(&lang) }.render()?).into_response())
}

pub async fn messages_partial(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    if db::chat::get_conversation(&state.pool, id, auth.user.id).await?.is_none() {
        return Err(crate::error::AppError::NotFound("404".into()));
    }
    let msgs = db::chat::list_messages(&state.pool, id, 100).await?;
    let now = OffsetDateTime::now_utc();
    let views = to_msg_views(&msgs, now, &auth.user.lang);
    Ok(Html(MessagesPartial { messages: views, t: crate::i18n::Strings::for_lang(&auth.user.lang) }.render()?).into_response())
}

#[derive(Template)]
#[template(path = "partials/messages.html")]
pub struct MessagesPartial {
    pub messages: Vec<MsgView>,
    pub t: &'static crate::i18n::Strings,
}

// ============== Tao / sua / xoa cuoc tro chuyen ==============

#[derive(Deserialize)]
pub struct CreateConvBody {
    pub title: Option<String>,
}

pub async fn create_conversation(
    State(state): State<AppState>,
    auth: AuthUser,
    body: Option<axum::Json<CreateConvBody>>,
) -> AppResult<Response> {
    let title = match body {
        Some(axum::Json(b)) => b.title,
        None => None,
    };
    let title = title
        .map(|t| truncate_chars(t.trim(), 120))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Cuộc trò chuyện mới".into());
    let conv = db::chat::create_conversation(&state.pool, auth.user.id, &title).await?;
    Ok(axum::Json(json!({ "id": conv.id })).into_response())
}

#[derive(Deserialize)]
pub struct RenameBody {
    pub title: String,
}

pub async fn rename_conversation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    body: Option<axum::Json<RenameBody>>,
) -> AppResult<Response> {
    let title = match body {
        Some(axum::Json(b)) => b.title,
        None => String::new(),
    };
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err(crate::error::AppError::BadRequest("Tiêu đề không được để trống.".into()));
    }
    db::chat::rename_conversation(&state.pool, id, auth.user.id, &truncate_chars(&title, 120)).await?;
    Ok((
        [("HX-Trigger", "refresh-conversations")],
        axum::Json(json!({ "ok": true })),
    )
        .into_response())
}

pub async fn delete_conversation(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    db::chat::delete_conversation(&state.pool, id, auth.user.id).await?;
    Ok((
        [("HX-Trigger", "refresh-conversations")],
        StatusCode::OK,
    )
        .into_response())
}

// ============== Mức dùng ==============

/// Trạng thái mức dùng hiện tại dưới dạng JSON.
pub async fn usage_json(State(state): State<AppState>, auth: AuthUser) -> Response {
    let snap = usage::snapshot(&state.pool, auth.user.id).await;
    axum::Json(snap.to_json()).into_response()
}

/// Phản hồi 429 kèm thông tin mức dùng để giao diện vẽ thanh usage.
fn usage_denied(snap: usage::Snapshot, t: &crate::i18n::Strings) -> Response {
    let resets = snap
        .resets_at
        .map(|r| {
            let local = r.to_offset(time::macros::offset!(+7));
            format!("{} {}", t.resets_at, crate::i18n::clock_time(local, t.lang))
        })
        .unwrap_or_default();
    let message = format!("{} · {}", t.limit_reached, resets);
    (
        StatusCode::TOO_MANY_REQUESTS,
        axum::Json(json!({
            "error": "usage_limit",
            "message": message,
            "usage": snap.to_json(),
        })),
    )
        .into_response()
}

// ============== Gui tin nhan (SSE) ==============

#[derive(Deserialize)]
pub struct SendMessageBody {
    pub conversation_id: Option<Uuid>,
    pub content: Option<String>,
}

pub async fn send_message(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::Json(body): axum::Json<SendMessageBody>,
) -> Response {
    let lang = auth.user.lang.clone();
    let t = crate::i18n::Strings::for_lang(&lang);
    let content = body.content.unwrap_or_default().trim().to_string();
    if content.is_empty() {
        return error_page(StatusCode::BAD_REQUEST, "400", t.msg_empty);
    }
    if content.chars().count() > 32_000 {
        return error_page(StatusCode::BAD_REQUEST, "400", t.msg_too_long);
    }

    // Mức dùng hàng giờ: chặn khi đã đạt giới hạn
    let usage_snap = match usage::check_and_record(&state.pool, auth.user.id).await {
        usage::Decision::Deny(snap) => return usage_denied(snap, t),
        usage::Decision::Allow(snap) => snap,
    };

    // Gioi han toc do: 20 tin / phut / nguoi dung
    if !state.rl.allow(auth.user.id, 20, Duration::from_secs(60)) {
        return error_page(StatusCode::TOO_MANY_REQUESTS, "429", t.too_fast);
    }

    // Tim hoac tao cuoc tro chuyen
    let conv_id = match body.conversation_id {
        Some(id) => match db::chat::get_conversation(&state.pool, id, auth.user.id).await {
            Ok(Some(c)) => c.id,
            _ => return error_page(StatusCode::NOT_FOUND, "404", t.conv_not_found),
        },
        None => match db::chat::create_conversation(&state.pool, auth.user.id, "Cuộc trò chuyện mới").await {
            Ok(c) => c.id,
            Err(e) => {
                tracing::error!("tao conv loi: {e}");
                return error_page(StatusCode::INTERNAL_SERVER_ERROR, "500", "Không tạo được cuộc trò chuyện.");
            }
        },
    };

    // Luu tin nhan cua nguoi dung
    if let Err(e) = db::chat::insert_message(
        &state.pool,
        conv_id,
        db::chat::NewMessage {
            role: "user",
            content: &content,
            thinking: None,
            tools: None,
            finish_reason: None,
            duration_ms: None,
        },
    )
    .await
    {
        tracing::error!("luu tin nhan loi: {e}");
        return error_page(StatusCode::INTERNAL_SERVER_ERROR, "500", "Không lưu được tin nhắn.");
    }
    let _ = db::chat::touch_conversation(&state.pool, conv_id).await;

    // Tu dong dat tieu de tu tin nhan dau tien
    let conv = db::chat::get_conversation(&state.pool, conv_id, auth.user.id).await.ok().flatten();
    if let Some(c) = conv {
        if c.title == "Cuộc trò chuyện mới" {
            let _ = db::chat::rename_conversation(&state.pool, conv_id, auth.user.id, &truncate_chars(&content, 48)).await;
        }
    }

    stream_response(state, conv_id, auth.user.id, lang, usage_snap).await
}

pub async fn regenerate(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::Json(body): axum::Json<SendMessageBody>,
) -> Response {
    let lang = auth.user.lang.clone();
    let t = crate::i18n::Strings::for_lang(&lang);
    let Some(conv_id) = body.conversation_id else {
        return error_page(StatusCode::BAD_REQUEST, "400", "Cần có conversation_id.");
    };
    if db::chat::get_conversation(&state.pool, conv_id, auth.user.id).await.ok().flatten().is_none() {
        return error_page(StatusCode::NOT_FOUND, "404", t.conv_not_found);
    }

    let usage_snap = match usage::check_and_record(&state.pool, auth.user.id).await {
        usage::Decision::Deny(snap) => return usage_denied(snap, t),
        usage::Decision::Allow(snap) => snap,
    };
    if !state.rl.allow(auth.user.id, 20, Duration::from_secs(60)) {
        return error_page(StatusCode::TOO_MANY_REQUESTS, "429", t.too_fast);
    }
    // Xoa phan tra loi cu
    let _ = db::chat::delete_after_last_user(&state.pool, conv_id).await;
    stream_response(state, conv_id, auth.user.id, lang, usage_snap).await
}

/// Tao SSE response; worker chay agent loop o background.
async fn stream_response(
    state: AppState,
    conv_id: Uuid,
    user_id: Uuid,
    lang: String,
    initial_usage: usage::Snapshot,
) -> Response {
    // He thong prompt tu settings
    let system_prompt = db::misc::get_setting(&state.pool, "system_prompt")
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Bạn là Halis, trợ lý AI của HalisAI.".into());

    let (tx, rx) = mpsc::channel::<AgentEvent>(256);

    let pool = state.pool.clone();
    let ai = state.ai.clone();
    let tools = state.tools.clone();
    let usage_lang = lang.clone();
    tokio::spawn(async move {
        // Trạng thái usage sau khi ghi nhận lượt này
        let _ = tx.send(AgentEvent::Usage(initial_usage)).await;

        let started = std::time::Instant::now();
        match ai::run_turn(&ai, &tools, &pool, conv_id, &system_prompt, &usage_lang, tx.clone()).await {
            Ok(outcome) => {
                let duration = started.elapsed().as_millis() as u64;
                let truncated = outcome.finish_reason == "length";
                let message_id = ai::save_turn(&pool, conv_id, &outcome, duration).await;
                let _ = tx
                    .send(AgentEvent::Done { truncated, duration_ms: duration })
                    .await;
                let _ = message_id;
            }
            Err(ai::TurnError::RateLimited) => {
                // Gateway 429: ép mức dùng về 100% và khoá (chỉ một lần)
                let snap = usage::force_block(&pool, user_id).await;
                let t = crate::i18n::Strings::for_lang(&usage_lang);
                let resets = snap
                    .resets_at
                    .map(|r| {
                        let local = r.to_offset(time::macros::offset!(+7));
                        format!("{} {}", t.resets_at, crate::i18n::clock_time(local, t.lang))
                    })
                    .unwrap_or_default();
                let _ = tx.send(AgentEvent::Usage(snap)).await;
                let _ = tx.send(AgentEvent::Error(format!("{} · {}", t.limit_reached, resets))).await;
            }
            Err(_) => {
                // Loi chi tiet da log o lop duoi; gui thong bao chung chung
                let _ = tx.send(AgentEvent::Error(ai::generic_error_message(&usage_lang))).await;
            }
        }
    });

    let stream = ReceiverStream::new(rx).filter_map(|ev| async move {
        let sse_event: Option<Event> = match ev {
            AgentEvent::Delta(t) => Some(
                Event::default()
                    .event("delta")
                    .data(json!({ "text": t }).to_string()),
            ),
            AgentEvent::Thinking(t) => Some(
                Event::default()
                    .event("thinking")
                    .data(json!({ "text": t }).to_string()),
            ),
            AgentEvent::ToolStart { name } => Some(
                Event::default()
                    .event("tool")
                    .data(json!({ "name": name, "status": "start" }).to_string()),
            ),
            AgentEvent::ToolDone { name, ms, summary } => Some(
                Event::default()
                    .event("tool")
                    .data(json!({ "name": name, "status": "done", "ms": ms, "summary": summary }).to_string()),
            ),
            AgentEvent::Usage(snap) => Some(
                Event::default()
                    .event("usage")
                    .data(snap.to_json().to_string()),
            ),
            AgentEvent::Done { truncated, duration_ms } => Some(
                Event::default()
                    .event("done")
                    .data(json!({ "truncated": truncated, "duration_ms": duration_ms }).to_string()),
            ),
            AgentEvent::Error(msg) => Some(
                Event::default()
                    .event("error")
                    .data(json!({ "message": msg }).to_string()),
            ),
        };
        sse_event.map(Ok::<_, Infallible>)
    });

    let headers = [
        ("Cache-Control", "no-cache"),
        ("X-Accel-Buffering", "no"),
        ("HX-Trigger", "refresh-conversations"),
    ];
    (
        headers,
        Sse::new(stream).keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("ping"),
        ),
    )
        .into_response()
}

pub async fn ping(auth: AuthUser) -> StatusCode {
    let _ = auth;
    StatusCode::NO_CONTENT
}
