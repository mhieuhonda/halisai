use askama::Template;
use axum::extract::{Query, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::db;
use crate::error::error_page;
use crate::state::AppState;
use crate::util::hash_token;

const OAUTH_STATE_COOKIE: &str = "halis_oauth_state";

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage {
    error: Option<String>,
}

#[derive(Deserialize)]
pub struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn soft_user(state: &AppState, jar: &CookieJar) -> Option<db::user::User> {
    let token = jar.get(auth::SESSION_COOKIE)?.value().to_string();
    let (_, user) = db::session::session_user(&state.pool, &hash_token(&token)).await.ok()??;
    Some(user)
}

pub async fn login_page(State(state): State<AppState>, jar: CookieJar) -> Response {
    if let Some(user) = soft_user(&state, &jar).await {
        if !user.is_banned(OffsetDateTime::now_utc()) {
            return Redirect::to("/").into_response();
        }
    }
    let missing_creds = state.cfg.google_client_id.is_empty();
    Html(
        LoginPage {
            error: if missing_creds {
                Some("Ứng dụng chưa cấu hình Google OAuth. Vui lòng liên hệ quản trị viên.".into())
            } else {
                None
            },
        }
        .render()
        .unwrap_or_default(),
    )
    .into_response()
}

pub async fn google_start(State(state): State<AppState>, _jar: CookieJar) -> Response {
    if state.cfg.google_client_id.is_empty() {
        return error_page(StatusCode::SERVICE_UNAVAILABLE, "Chưa cấu hình", "Google OAuth chưa được cấu hình trên máy chủ.");
    }
    let oauth_state = Uuid::new_v4().simple().to_string();
    let redirect_uri = format!("{}/auth/google/callback", state.cfg.base_url);
    let url = crate::auth::google::authorize_url(&state.cfg.google_client_id, &redirect_uri, &oauth_state);
    let mut cookie = format!("{OAUTH_STATE_COOKIE}={oauth_state}; Path=/; HttpOnly; SameSite=Lax; Max-Age=600");
    if state.cfg.secure_cookies {
        cookie.push_str("; Secure");
    }
    (
        [(SET_COOKIE, HeaderValue::from_str(&cookie).expect("cookie hop le"))],
        Redirect::to(&url),
    )
        .into_response()
}

pub async fn google_callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(params): Query<CallbackParams>,
) -> Response {
    let fail = |msg: &str| error_page(StatusCode::BAD_REQUEST, "Đăng nhập thất bại", msg);

    if params.error.is_some() {
        return Redirect::to("/login").into_response();
    }
    let Some(code) = params.code else {
        return fail("Thiếu mã xác thực từ Google.");
    };
    let Some(oauth_state) = params.state else {
        return fail("Thiếu state.");
    };
    let stored = jar.get(OAUTH_STATE_COOKIE).map(|c| c.value().to_string());
    if stored.as_deref() != Some(oauth_state.as_str()) {
        return fail("Phiên đăng nhập không hợp lệ, vui lòng thử lại.");
    }

    let redirect_uri = format!("{}/auth/google/callback", state.cfg.base_url);
    let tokens = match crate::auth::google::exchange_code(
        &state.http,
        &code,
        &state.cfg.google_client_id,
        &state.cfg.google_client_secret,
        &redirect_uri,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("exchange code loi: {e}");
            return fail("Không trao đổi được mã với Google. Vui lòng thử lại.");
        }
    };

    let identity = match crate::auth::google::verify_id_token(
        &state.http,
        &tokens.id_token,
        &state.cfg.google_client_id,
    )
    .await
    {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!("verify id_token loi: {e}");
            return fail("Không xác minh được danh tính từ Google.");
        }
    };

    let user = match db::user::upsert_google_user(
        &state.pool,
        &identity.sub,
        &identity.email.to_lowercase(),
        &identity.name,
        identity.picture.as_deref(),
    )
    .await
    {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("upsert user loi: {e}");
            return fail("Không lưu được thông tin tài khoản.");
        }
    };

    // Bootstrap: nguoi dau tien hoac email trong ADMIN_EMAILS thanh admin
    let is_admin_email = state.cfg.admin_emails.iter().any(|e| *e == user.email.to_lowercase());
    if is_admin_email && user.role != "admin" {
        let _ = db::user::set_role(&state.pool, user.id, "admin").await;
    } else if db::user::count_users(&state.pool).await.unwrap_or(1) <= 1 && user.role == "user" {
        tracing::info!("nguoi dung dau tien {} duoc len admin", user.email);
        let _ = db::user::set_role(&state.pool, user.id, "admin").await;
    }

    if user.is_banned(OffsetDateTime::now_utc()) {
        return Redirect::to("/banned").into_response();
    }

    start_session(&state, user.id).await
}

pub async fn start_session(state: &AppState, user_id: Uuid) -> Response {
    let token = Uuid::new_v4().to_string();
    let csrf = Uuid::new_v4().simple().to_string();
    let expires = OffsetDateTime::now_utc() + time::Duration::days(30);
    if let Err(e) = db::session::create_session(
        &state.pool,
        user_id,
        &hash_token(&token),
        &csrf,
        None,
        None,
        expires,
    )
    .await
    {
        tracing::error!("tao session loi: {e}");
        return error_page(StatusCode::INTERNAL_SERVER_ERROR, "Lỗi hệ thống", "Không tạo được phiên đăng nhập.");
    }
    (
        [(SET_COOKIE, auth::session_cookie(&token, state.cfg.secure_cookies))],
        Redirect::to("/"),
    )
        .into_response()
}

pub async fn logout(State(state): State<AppState>, _auth: AuthUser, jar: CookieJar) -> Response {
    if let Some(cookie) = jar.get(auth::SESSION_COOKIE) {
        let _ = db::session::delete_session(&state.pool, &hash_token(cookie.value())).await;
    }
    (
        [(SET_COOKIE, auth::clear_session_cookie())],
        [("HX-Redirect", "/login")],
        Redirect::to("/login"),
    )
        .into_response()
}

#[derive(Template)]
#[template(path = "banned.html")]
struct BannedPage {
    reason: Option<String>,
    until: Option<String>,
    permanent: bool,
}

pub async fn banned_page(State(state): State<AppState>, jar: CookieJar) -> Response {
    let Some(user) = soft_user(&state, &jar).await else {
        return Redirect::to("/login").into_response();
    };
    if !user.is_banned(OffsetDateTime::now_utc()) {
        return Redirect::to("/").into_response();
    }
    let until = user
        .banned_until
        .map(crate::util::format_full);
    Html(
        BannedPage {
            reason: user.ban_reason.clone(),
            until,
            permanent: user.ban_permanent,
        }
        .render()
        .unwrap_or_default(),
    )
    .into_response()
}

#[derive(Deserialize)]
pub struct DevLoginParams {
    name: Option<String>,
    role: Option<String>,
}

/// Chi hoat dong khi DEV_LOGIN=true - tao user gia de test UI.
pub async fn dev_login(State(state): State<AppState>, Query(params): Query<DevLoginParams>) -> Response {
    if !state.cfg.dev_login {
        return error_page(StatusCode::NOT_FOUND, "Không tìm thấy", "Trang không tồn tại.");
    }
    let name: String = params
        .name
        .unwrap_or_else(|| "Dev Tester".into())
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .take(40)
        .collect::<String>()
        .trim()
        .to_string();
    let name = if name.is_empty() { "Dev Tester".to_string() } else { name };
    let email = format!("dev.{}@halisai.local", name.to_lowercase().replace(' ', "."));
    let sub = format!("dev:{email}");

    let user = match db::user::upsert_google_user(&state.pool, &sub, &email, &name, None).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("dev login loi: {e}");
            return error_page(StatusCode::INTERNAL_SERVER_ERROR, "Lỗi", "Không tạo được user dev.");
        }
    };
    // Cho phep chi dinh role khi test
    if let Some(role) = params.role {
        if matches!(role.as_str(), "user" | "moderator" | "admin") && role != user.role {
            let _ = db::user::set_role(&state.pool, user.id, &role).await;
        }
    }
    tracing::warn!("dev login: {} ({})", user.email, user.role);
    start_session(&state, user.id).await
}

pub async fn healthz() -> Response {
    StatusCode::OK.into_response()
}

pub async fn not_found() -> Response {
    error_page(StatusCode::NOT_FOUND, "404", "Trang bạn tìm không tồn tại.")
}
