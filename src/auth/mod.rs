pub mod google;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use time::OffsetDateTime;

use crate::db::{self, user::User};
use crate::error::error_page;
use crate::state::AppState;
use crate::util::hash_token;

pub const SESSION_COOKIE: &str = "halis_session";

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user: User,
    pub csrf: String,
}

fn redirect_login() -> Response {
    Redirect::to("/login").into_response()
}

async fn resolve(parts: &mut Parts, state: &AppState) -> Result<AuthUser, Box<Response>> {
    let jar = CookieJar::from_request_parts(parts, state)
        .await
        .map_err(|_| Box::new(redirect_login()))?;
    let Some(cookie) = jar.get(SESSION_COOKIE) else {
        return Err(Box::new(redirect_login()));
    };
    let hash = hash_token(cookie.value());
    let Some((_session, user)) = db::session::session_user(&state.pool, &hash).await.ok().flatten()
    else {
        return Err(Box::new(redirect_login()));
    };

    let now = OffsetDateTime::now_utc();
    if user.is_banned(now) {
        return Err(Box::new(Redirect::to("/banned").into_response()));
    }

    // CSRF: moi request lam thay doi du lieu phai kem token hop le.
    let method = parts.method.clone();
    let safe = method.is_safe() || method == "OPTIONS";
    let path = parts.uri.path();
    if !safe && !path.starts_with("/auth") && path != "/dev/login" {
        let header = parts
            .headers
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if header.is_empty() || header != _session.csrf {
            return Err(Box::new(error_page(
                StatusCode::FORBIDDEN,
                "Phiên không hợp lệ",
                "Token bảo mật không khớp. Vui lòng tải lại trang và thử lại.",
            )));
        }
    }

    // Cap nhat hoat dong (throttle 30s ben duoi db)
    let _ = db::user::touch_last_seen(&state.pool, user.id).await;
    let _ = db::session::touch_session(&state.pool, _session.id).await;

    Ok(AuthUser { user, csrf: _session.csrf })
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        resolve(parts, state).await.map_err(|r| *r)
    }
}

/// Yeu cau dang nhap va co role moderator tro len.
pub struct StaffUser(pub AuthUser);

impl FromRequestParts<AppState> for StaffUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let auth = resolve(parts, state).await.map_err(|r| *r)?;
        if !auth.user.is_staff() {
            return Err(error_page(
                StatusCode::FORBIDDEN,
                "Không đủ quyền",
                "Bạn không có quyền truy cập khu vực quản trị.",
            ));
        }
        Ok(StaffUser(auth))
    }
}

/// Yeu cau role admin.
pub struct AdminUser(pub AuthUser);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let auth = resolve(parts, state).await.map_err(|r| *r)?;
        if !auth.user.is_admin() {
            return Err(error_page(
                StatusCode::FORBIDDEN,
                "Không đủ quyền",
                "Chỉ administrator mới được thực hiện thao tác này.",
            ));
        }
        Ok(AdminUser(auth))
    }
}

/// Tao cookie session (HttpOnly, SameSite=Lax).
pub fn session_cookie(token: &str, secure: bool) -> HeaderValue {
    let mut cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        30 * 24 * 3600
    );
    if secure {
        cookie.push_str("; Secure");
    }
    HeaderValue::from_str(&cookie).expect("cookie hop le")
}

pub fn clear_session_cookie() -> HeaderValue {
    HeaderValue::from_str(&format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"))
        .expect("cookie hop le")
}
