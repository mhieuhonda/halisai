use askama::Template;
use axum::extract::State;
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::db;
use crate::error::AppResult;
use crate::routes::build_ctx;
use crate::state::AppState;

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsPage {
    pub ctx: crate::routes::Ctx,
    pub active: Option<crate::routes::chat::ActiveConv>,
}

pub async fn page(State(state): State<AppState>, auth: AuthUser) -> AppResult<Response> {
    let ctx = build_ctx(&state, &auth).await;
    Ok(Html(SettingsPage { ctx, active: None }.render()?).into_response())
}

#[derive(Deserialize)]
pub struct PrefsBody {
    pub lang: Option<String>,
    pub theme: Option<String>,
}

/// Lưu tùy chọn ngôn ngữ / chủ đề. Trả về trạng thái sau khi lưu.
pub async fn save_prefs(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::Json(body): axum::Json<PrefsBody>,
) -> Response {
    let lang = body
        .lang
        .filter(|l| l == "vi" || l == "en")
        .unwrap_or_else(|| auth.user.lang.clone());
    let theme = body
        .theme
        .filter(|t| t == "light" || t == "dark")
        .unwrap_or_else(|| auth.user.theme.clone());

    if let Err(e) = db::user::set_prefs(&state.pool, auth.user.id, &lang, &theme).await {
        tracing::error!("luu prefs loi: {e}");
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "ok": false })),
        )
            .into_response();
    }
    let _ = db::misc::audit(
        &state.pool,
        &auth.user,
        "prefs.update",
        Some(&auth.user.email),
        Some(serde_json::json!({ "lang": lang, "theme": theme })),
    )
    .await;

    (
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({ "ok": true, "lang": lang, "theme": theme })),
    )
        .into_response()
}
