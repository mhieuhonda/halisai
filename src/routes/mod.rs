pub mod admin;
pub mod chat;
pub mod login;

use askama::Template;
use axum::routing::{get, post};
use axum::Router;

use crate::auth::AuthUser;
use crate::state::AppState;

/// Du lieu chung cho moi trang.
#[derive(Clone, Debug)]
#[allow(dead_code)]
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
    }
}

#[derive(Template)]
#[template(path = "partials/conversations.html")]
pub struct ConversationsPartial {
    pub convs: Vec<ConvItem>,
}

#[derive(Clone, Debug)]
pub struct ConvItem {
    pub id: uuid::Uuid,
    pub title: String,
    pub rel: String,
    pub active: bool,
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
        .route("/api/ping", get(chat::ping))
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
