use askama::Template;
use axum::extract::{Form, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::auth::{AdminUser, StaffUser};
use crate::db::{self, misc, user::User};
use crate::error::{error_page, AppError, AppResult};
use crate::routes::{build_ctx, Ctx};
use crate::state::AppState;
use crate::i18n;
use crate::util::truncate_chars;

const PAGE_SIZE: i64 = 20;

// ==================== Dashboard ====================

#[derive(Template)]
#[template(path = "admin/dashboard.html")]
pub struct DashboardPage {
    pub ctx: Ctx,
    pub stats: Stats,
    pub chart: Vec<DayCount>,
    pub recent_users: Vec<UserRow>,
    pub recent_audit: Vec<AuditRow>,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub users: i64,
    pub online: i64,
    pub conversations: i64,
    pub messages_24h: i64,
    pub messages_total: i64,
}

#[derive(Clone, Debug)]
pub struct DayCount {
    pub day: String,
    pub count: i64,
    pub pct: i64,
}

pub async fn dashboard(State(state): State<AppState>, staff: StaffUser) -> AppResult<Response> {
    let auth = staff.0;
    let ctx = build_ctx(&state, &auth).await;
    let since = OffsetDateTime::now_utc() - time::Duration::hours(24);
    let stats = Stats {
        users: db::user::count_users(&state.pool).await?,
        online: misc::count_online_users(&state.pool).await?,
        conversations: db::chat::count_conversations(&state.pool).await?,
        messages_24h: db::chat::count_messages_since(&state.pool, since).await?,
        messages_total: db::chat::count_messages(&state.pool).await?,
    };
    let raw_chart = db::chat::messages_per_day(&state.pool, 14).await?;
    let max_count = raw_chart.iter().map(|(_, c)| *c).max().unwrap_or(1).max(1);
    let chart = raw_chart
        .into_iter()
        .map(|(day, count)| DayCount {
            day,
            count,
            pct: if count == 0 { 3 } else { (count * 100 / max_count).max(6) },
        })
        .collect();
    let now = OffsetDateTime::now_utc();
    let recent_users = misc::recent_users(&state.pool, 6)
        .await?
        .into_iter()
        .map(|u| UserRow::from_user(u, now))
        .collect();
    let recent_audit = misc::list_audit(&state.pool, 8, 0)
        .await?
        .into_iter()
        .map(AuditRow::from)
        .collect();
    Ok(Html(DashboardPage { ctx, stats, chart, recent_users, recent_audit }.render()?).into_response())
}

// ==================== Users ====================

#[derive(Clone, Debug)]
pub struct UserRow {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub picture: Option<String>,
    pub initial: String,
    pub role: String,
    pub online: bool,
    pub banned: bool,
    pub ban_permanent: bool,
    pub ban_until: Option<String>,
    pub ban_reason: Option<String>,
    pub created_rel: String,
    pub last_seen_rel: String,
    pub is_self: bool,
}

impl UserRow {
    fn from_user(u: User, now: OffsetDateTime) -> Self {
        let ban_until = u.banned_until.map(i18n::format_full_vi);
        let initial = u.name.chars().next().map(|c| c.to_string()).unwrap_or_else(|| "?".into());
        let online = u.is_online(now);
        let banned = u.is_banned(now);
        Self {
            id: u.id,
            name: u.name,
            email: u.email,
            picture: u.picture,
            initial,
            role: u.role,
            online,
            banned,
            ban_permanent: u.ban_permanent,
            ban_until,
            ban_reason: u.ban_reason,
            created_rel: i18n::relative_time(u.created_at, now, "vi"),
            last_seen_rel: i18n::relative_time(u.last_seen, now, "vi"),
            is_self: false,
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct AuditRow {
    pub actor: String,
    pub action: String,
    pub action_label: String,
    pub target: Option<String>,
    pub details: Option<String>,
    pub rel: String,
}

impl From<misc::AuditEntry> for AuditRow {
    fn from(e: misc::AuditEntry) -> Self {
        let now = OffsetDateTime::now_utc();
        let details = e
            .details
            .and_then(|d| match d {
                serde_json::Value::Object(map) => {
                    let parts: Vec<String> = map
                        .iter()
                        .map(|(k, v)| format!("{k}: {}", compact_json(v)))
                        .collect();
                    if parts.is_empty() { None } else { Some(parts.join(" · ")) }
                }
                other => Some(compact_json(&other)),
            })
            .map(|s| truncate_chars(&s, 160));
        Self {
            actor: e.actor_email.unwrap_or_else(|| "—".into()),
            action: e.action.clone(),
            action_label: label_action(&e.action),
            target: e.target,
            details,
            rel: i18n::relative_time(e.created_at, now, "vi"),
        }
    }
}

fn compact_json(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn label_action(action: &str) -> String {
    match action {
        "user.role" => "Đổi quyền",
        "user.ban" => "Cấm tài khoản",
        "user.unban" => "Bỏ cấm",
        "user.kick" => "Đăng xuất từ xa",
        "user.delete" => "Xóa tài khoản",
        "settings.update" => "Cập nhật cấu hình",
        _ => "Thao tác khác",
    }
    .to_string()
}

#[derive(Template)]
#[template(path = "admin/users.html")]
pub struct UsersPage {
    pub ctx: Ctx,
    pub q: String,
    pub page: i64,
    pub total: i64,
    pub users: Vec<UserRow>,
    pub viewer_role: String,
}

#[derive(Template)]
#[template(path = "partials/admin_users.html")]
pub struct UsersTable {
    pub q: String,
    pub page: i64,
    pub total: i64,
    pub users: Vec<UserRow>,
    pub viewer_role: String,
}

async fn users_data(state: &AppState, viewer: &User, q: &str, page: i64) -> AppResult<UsersTable> {
    let total = db::user::count_users(&state.pool).await?;
    let users = db::user::list_users(&state.pool, q, PAGE_SIZE, (page - 1).max(0) * PAGE_SIZE).await?;
    let now = OffsetDateTime::now_utc();
    let users = users
        .into_iter()
        .map(|u| {
            let is_self = u.id == viewer.id;
            let mut row = UserRow::from_user(u, now);
            row.is_self = is_self;
            row
        })
        .collect();
    Ok(UsersTable { q: q.to_string(), page, total, users, viewer_role: viewer.role.clone() })
}

pub async fn users_page(
    State(state): State<AppState>,
    staff: StaffUser,
    Query(q): Query<SearchQuery>,
) -> AppResult<Response> {
    let auth = staff.0;
    let ctx = build_ctx(&state, &auth).await;
    let page = q.page.unwrap_or(1).max(1);
    let table = users_data(&state, &auth.user, &q.q.unwrap_or_default(), page).await?;
    Ok(Html(
        UsersPage {
            ctx,
            q: table.q.clone(),
            page: table.page,
            total: table.total,
            users: table.users,
            viewer_role: table.viewer_role.clone(),
        }
        .render()?,
    )
    .into_response())
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub page: Option<i64>,
}

pub async fn users_table(
    State(state): State<AppState>,
    staff: StaffUser,
    Query(q): Query<SearchQuery>,
) -> AppResult<Response> {
    let auth = staff.0;
    let page = q.page.unwrap_or(1).max(1);
    let table = users_data(&state, &auth.user, &q.q.unwrap_or_default(), page).await?;
    Ok(Html(table.render()?).into_response())
}

/// Tra ve bang users moi sau moi thao tac (hx-target #users-wrap).
async fn respond_users_table(state: &AppState, viewer: &User, q: &str, page: i64) -> Response {
    match users_data(state, viewer, q, page).await {
        Ok(t) => Html(t.render().unwrap_or_default()).into_response(),
        Err(e) => e.into_response(),
    }
}

// ==================== Hanh dong quan tri ====================

#[derive(Deserialize)]
pub struct RoleForm {
    pub role: String,
    pub q: String,
    pub page: Option<i64>,
}

pub async fn set_role(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<RoleForm>,
) -> AppResult<Response> {
    let viewer = admin.0;
    if !matches!(form.role.as_str(), "user" | "moderator" | "admin") {
        return Err(AppError::BadRequest("Quyền không hợp lệ.".into()));
    }
    if id == viewer.user.id {
        return Err(AppError::BadRequest("Không thể đổi quyền của chính mình.".into()));
    }
    let Some(target) = db::user::get_user(&state.pool, id).await? else {
        return Err(AppError::NotFound("Người dùng không tồn tại.".into()));
    };
    db::user::set_role(&state.pool, id, &form.role).await?;
    misc::audit(
        &state.pool,
        &viewer.user,
        "user.role",
        Some(&target.email),
        Some(json!({ "from": target.role, "to": form.role })),
    )
    .await?;
    Ok(respond_users_table(&state, &viewer.user, &form.q, form.page.unwrap_or(1)).await)
}

#[derive(Deserialize)]
pub struct BanForm {
    pub duration: String,
    pub reason: String,
    pub q: String,
    pub page: Option<i64>,
}

pub async fn ban_user(
    State(state): State<AppState>,
    staff: StaffUser,
    Path(id): Path<Uuid>,
    Form(form): Form<BanForm>,
) -> AppResult<Response> {
    let viewer = staff.0;
    let Some(target) = db::user::get_user(&state.pool, id).await? else {
        return Err(AppError::NotFound("Người dùng không tồn tại.".into()));
    };
    if target.rank() >= viewer.user.rank() {
        return Err(AppError::Forbidden("Không thể cấm người có quyền ngang hoặc cao hơn.".into()));
    }
    let (until, permanent) = match form.duration.as_str() {
        "1h" => (Some(OffsetDateTime::now_utc() + time::Duration::hours(1)), false),
        "24h" => (Some(OffsetDateTime::now_utc() + time::Duration::hours(24)), false),
        "7d" => (Some(OffsetDateTime::now_utc() + time::Duration::days(7)), false),
        "30d" => (Some(OffsetDateTime::now_utc() + time::Duration::days(30)), false),
        "forever" => (None, true),
        _ => return Err(AppError::BadRequest("Thời hạn không hợp lệ.".into())),
    };
    let reason = form.reason.trim();
    let reason = if reason.is_empty() { None } else { Some(truncate_chars(reason, 300)) };
    db::user::set_ban(&state.pool, id, until, permanent, reason.as_deref()).await?;
    let _ = db::session::delete_user_sessions(&state.pool, id).await;
    misc::audit(
        &state.pool,
        &viewer.user,
        "user.ban",
        Some(&target.email),
        Some(json!({
            "duration": form.duration,
            "reason": reason.unwrap_or_default(),
            "until": until.map(crate::util::to_rfc3339).unwrap_or_else(|| "vĩnh viễn".into())
        })),
    )
    .await?;
    Ok(respond_users_table(&state, &viewer.user, &form.q, form.page.unwrap_or(1)).await)
}

#[derive(Deserialize)]
pub struct SimpleActionForm {
    pub q: String,
    pub page: Option<i64>,
}

pub async fn unban_user(
    State(state): State<AppState>,
    staff: StaffUser,
    Path(id): Path<Uuid>,
    Form(form): Form<SimpleActionForm>,
) -> AppResult<Response> {
    let viewer = staff.0;
    let Some(target) = db::user::get_user(&state.pool, id).await? else {
        return Err(AppError::NotFound("Người dùng không tồn tại.".into()));
    };
    if target.rank() >= viewer.user.rank() {
        return Err(AppError::Forbidden("Không đủ quyền.".into()));
    }
    db::user::set_ban(&state.pool, id, None, false, None).await?;
    misc::audit(&state.pool, &viewer.user, "user.unban", Some(&target.email), None).await?;
    Ok(respond_users_table(&state, &viewer.user, &form.q, form.page.unwrap_or(1)).await)
}

pub async fn kick_user(
    State(state): State<AppState>,
    staff: StaffUser,
    Path(id): Path<Uuid>,
    Form(form): Form<SimpleActionForm>,
) -> AppResult<Response> {
    let viewer = staff.0;
    let Some(target) = db::user::get_user(&state.pool, id).await? else {
        return Err(AppError::NotFound("Người dùng không tồn tại.".into()));
    };
    if target.rank() >= viewer.user.rank() {
        return Err(AppError::Forbidden("Không đủ quyền.".into()));
    }
    let kicked = db::session::delete_user_sessions(&state.pool, id).await?;
    misc::audit(
        &state.pool,
        &viewer.user,
        "user.kick",
        Some(&target.email),
        Some(json!({ "sessions": kicked })),
    )
    .await?;
    Ok(respond_users_table(&state, &viewer.user, &form.q, form.page.unwrap_or(1)).await)
}

pub async fn delete_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Form(form): Form<SimpleActionForm>,
) -> AppResult<Response> {
    let viewer = admin.0;
    if id == viewer.user.id {
        return Err(AppError::BadRequest("Không thể xóa chính mình.".into()));
    }
    let Some(target) = db::user::get_user(&state.pool, id).await? else {
        return Err(AppError::NotFound("Người dùng không tồn tại.".into()));
    };
    db::user::delete_user(&state.pool, id).await?;
    misc::audit(&state.pool, &viewer.user, "user.delete", Some(&target.email), None).await?;
    Ok(respond_users_table(&state, &viewer.user, &form.q, form.page.unwrap_or(1)).await)
}

// ==================== Audit log ====================

#[derive(Template)]
#[template(path = "admin/audit.html")]
pub struct AuditPage {
    pub ctx: Ctx,
    pub entries: Vec<AuditRow>,
    pub page: i64,
    pub total: i64,
}

pub async fn audit_page(
    State(state): State<AppState>,
    staff: StaffUser,
    Query(q): Query<SearchQuery>,
) -> AppResult<Response> {
    let ctx = build_ctx(&state, &staff.0).await;
    let page = q.page.unwrap_or(1).max(1);
    let total = misc::count_audit(&state.pool).await?;
    let entries = misc::list_audit(&state.pool, PAGE_SIZE, (page - 1) * PAGE_SIZE)
        .await?
        .into_iter()
        .map(AuditRow::from)
        .collect();
    Ok(Html(AuditPage { ctx, entries, page, total }.render()?).into_response())
}

// ==================== Settings ====================

#[derive(Template)]
#[template(path = "admin/settings.html")]
pub struct SettingsPage {
    pub ctx: Ctx,
    pub system_prompt: String,
    pub announcement: String,
    pub saved: bool,
}

#[derive(Template)]
#[template(path = "partials/admin_settings.html")]
pub struct SettingsPartial {
    pub system_prompt: String,
    pub announcement: String,
    pub saved: bool,
}

pub async fn settings_page(State(state): State<AppState>, admin: AdminUser) -> AppResult<Response> {
    let ctx = build_ctx(&state, &admin.0).await;
    let system_prompt = misc::get_setting(&state.pool, "system_prompt").await?.unwrap_or_default();
    let announcement = misc::get_setting(&state.pool, "announcement").await?.unwrap_or_default();
    Ok(Html(SettingsPage { ctx, system_prompt, announcement, saved: false }.render()?).into_response())
}

#[derive(Deserialize)]
pub struct SettingsForm {
    pub system_prompt: String,
    pub announcement: String,
}

pub async fn save_settings(
    State(state): State<AppState>,
    admin: AdminUser,
    Form(form): Form<SettingsForm>,
) -> AppResult<Response> {
    let viewer = admin.0;
    let prompt = form.system_prompt.trim().to_string();
    let announcement = form.announcement.trim().to_string();
    if prompt.len() < 20 {
        return Err(AppError::BadRequest("System prompt quá ngắn.".into()));
    }
    misc::set_setting(&state.pool, "system_prompt", &prompt, viewer.user.id).await?;
    misc::set_setting(&state.pool, "announcement", &announcement, viewer.user.id).await?;
    misc::audit(
        &state.pool,
        &viewer.user,
        "settings.update",
        None,
        Some(json!({ "announcement_len": announcement.len(), "prompt_len": prompt.len() })),
    )
    .await?;
    let page = SettingsPartial { system_prompt: prompt, announcement, saved: true };
    Ok(Html(page.render()?).into_response())
}

// ==================== 404 cho admin ====================
#[allow(dead_code)]
pub async fn admin_not_found() -> Response {
    error_page(StatusCode::NOT_FOUND, "404", "Không tìm thấy trong khu vực quản trị.")
}
