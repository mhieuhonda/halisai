use askama::Template;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

#[derive(Debug)]
pub enum AppError {
    Db(sqlx::Error),
    Template(askama::Error),
    BadRequest(String),
    NotFound(String),
    Forbidden(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Db(e) => write!(f, "loi db: {e}"),
            AppError::Template(e) => write!(f, "loi template: {e}"),
            AppError::BadRequest(m) => write!(f, "yeu cau khong hop le: {m}"),
            AppError::NotFound(m) => write!(f, "khong tim thay: {m}"),
            AppError::Forbidden(m) => write!(f, "khong du quyen: {m}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Db(e)
    }
}

impl From<askama::Error> for AppError {
    fn from(e: askama::Error) -> Self {
        AppError::Template(e)
    }
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPage {
    code: u16,
    title: String,
    message: String,
}

pub fn error_page(code: StatusCode, title: &str, message: &str) -> Response {
    let tpl = ErrorPage {
        code: code.as_u16(),
        title: title.to_string(),
        message: message.to_string(),
    };
    match tpl.render() {
        Ok(html) => (code, Html(html)).into_response(),
        Err(_) => (code, format!("{code}")).into_response(),
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Log day du phia server, khong roi le chi tiet ra ngoai
        match &self {
            AppError::Db(e) => tracing::error!("db error: {e:?}"),
            AppError::Template(e) => tracing::error!("template error: {e:?}"),
            _ => tracing::warn!("{self}"),
        }
        match self {
            AppError::BadRequest(m) => error_page(StatusCode::BAD_REQUEST, "Yêu cầu không hợp lệ", &m),
            AppError::NotFound(m) => error_page(StatusCode::NOT_FOUND, "Không tìm thấy", &m),
            AppError::Forbidden(m) => error_page(StatusCode::FORBIDDEN, "Không đủ quyền", &m),
            _ => error_page(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Lỗi hệ thống",
                "Đã có lỗi xảy ra phía máy chủ. Vui lòng thử lại sau.",
            ),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
