pub mod admin;
pub mod chat;
pub mod login;
pub mod settings;

use askama::Template;
use axum::routing::{get, post};
use axum::Router;

use crate::auth::AuthUser;
use crate::i18n;
use crate::state::AppState;
use crate::usage;

/// Du lieu chung cho moi trang.
#[derive(Clone, Debug)]
pub struct Ctx {
    pub csrf: String,
    pub user_name: String,
    pub user_initial: String,
    pub user_email: String,
    pub user_role: String,
    pub user_picture: Option<String>,
    pub announcement: Option<String>,
    pub ann_key: String,
    pub is_staff: bool,
    pub lang: String,
    pub theme: String,
    pub t: &'static i18n::Strings,
    pub usage: usage::Snapshot,
}

pub async fn build_ctx(state: &AppState, auth: &AuthUser) -> Ctx {
    let announcement = crate::db::misc::get_setting(&state.pool, "announcement")
        .await
        .ok()
        .flatten()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let ann_key = announcement
        .as_ref()
        .map(|s| crate::util::hash_token(s)[..12].to_string())
        .unwrap_or_default();
    let usage_snap = usage::snapshot(&state.pool, auth.user.id).await;
    Ctx {
        csrf: auth.csrf.clone(),
        user_name: auth.user.name.clone(),
        user_initial: auth
            .user
            .name
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".into()),
        user_email: auth.user.email.clone(),
        user_role: auth.user.role.clone(),
        user_picture: auth.user.picture.clone(),
        announcement,
        ann_key,
        is_staff: auth.user.is_staff(),
        lang: auth.user.lang.clone(),
        theme: auth.user.theme.clone(),
        t: i18n::Strings::for_lang(&auth.user.lang),
        usage: usage_snap,
    }
}

#[derive(Template)]
#[template(path = "partials/conversations.html")]
pub struct ConversationsPartial {
    pub groups: Vec<ConvGroup>,
    pub t: &'static i18n::Strings,
}

/// Nhóm cuộc trò chuyện theo thời gian.
#[derive(Clone, Debug)]
pub struct ConvGroup {
    pub label: String,
    pub convs: Vec<ConvItem>,
}

#[derive(Clone, Debug)]
pub struct ConvItem {
    pub id: uuid::Uuid,
    pub title: String,
    pub active: bool,
    pub updated_at: time::OffsetDateTime,
}

/// Xếp các cuộc trò chuyện vào nhóm theo ngày cập nhật (theo giờ GMT+7).
fn group_conversations(convs: Vec<ConvItem>, lang: &str) -> Vec<ConvGroup> {
    let now = crate::util::local_now();
    let today = now.date();
    let groups: [(&str, i64); 5] = [
        (i18n::Strings::for_lang(lang).today, 0),
        (i18n::Strings::for_lang(lang).yesterday, 1),
        (i18n::Strings::for_lang(lang).last7, 7),
        (i18n::Strings::for_lang(lang).last30, 30),
        (i18n::Strings::for_lang(lang).older, i64::MAX),
    ];
    let mut out: Vec<ConvGroup> = Vec::new();
    for (label, _days) in groups {
        out.push(ConvGroup { label: label.to_string(), convs: Vec::new() });
    }
    for c in convs {
        // updated_at đang là UTC; quy về ngày GMT+7 để so khớp hôm nay
        let local = c.updated_at.to_offset(time::macros::offset!(+7)).date();
        let diff_days = (today - local).whole_days();
        let idx = match diff_days {
            0 => 0,
            1 => 1,
            2..=7 => 2,
            8..=30 => 3,
            _ => 4,
        };
        out[idx].convs.push(c);
    }
    out.retain(|g| !g.convs.is_empty());
    out
}

pub fn router(state: AppState) -> Router {
    Router::new()
        // Chat
        .route("/", get(chat::chat_home))
        .route("/chat/{id}", get(chat::chat_page))
        .route("/chat/{id}/messages", get(chat::messages_partial))
        .route("/partials/conversations", get(chat::conversations_partial))
        .route("/api/conversations", post(chat::create_conversation))
        .route("/api/conversations/{id}/rename", post(chat::rename_conversation))
        .route("/api/conversations/{id}/delete", post(chat::delete_conversation))
        .route("/api/chat", post(chat::send_message))
        .route("/api/chat/regenerate", post(chat::regenerate))
        .route("/api/usage", get(chat::usage_json))
        .route("/api/ping", get(chat::ping))
        // Cài đặt người dùng
        .route("/settings", get(settings::page))
        .route("/api/settings/prefs", post(settings::save_prefs))
        // Auth
        .route("/login", get(login::login_page))
        .route("/auth/google", get(login::google_start))
        .route("/auth/google/callback", get(login::google_callback))
        .route("/auth/logout", post(login::logout))
        .route("/banned", get(login::banned_page))
        .route("/dev/login", post(login::dev_login))
        .route("/healthz", get(login::healthz))
        // Admin
        .route("/admin", get(admin::dashboard))
        .route("/admin/users", get(admin::users_page))
        .route("/partials/admin/users", get(admin::users_table))
        .route("/admin/users/{id}/role", post(admin::set_role))
        .route("/admin/users/{id}/ban", post(admin::ban_user))
        .route("/admin/users/{id}/unban", post(admin::unban_user))
        .route("/admin/users/{id}/kick", post(admin::kick_user))
        .route("/admin/users/{id}/delete", post(admin::delete_user))
        .route("/admin/audit", get(admin::audit_page))
        .route("/admin/settings", get(admin::settings_page))
        .route("/admin/settings", post(admin::save_settings))
        // Static
        .nest_service("/static", tower_http::services::ServeDir::new("static"))
        .fallback(login::not_found)
        .with_state(state)
}
