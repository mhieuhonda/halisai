use serde_json::Value as Json;
use sqlx::{query, query_as, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use super::user::User;

#[derive(Clone, Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct AuditEntry {
    pub id: i64,
    pub actor_email: Option<String>,
    pub action: String,
    pub target: Option<String>,
    pub details: Option<Json>,
    pub created_at: OffsetDateTime,
}

pub async fn audit(
    pool: &PgPool,
    actor: &User,
    action: &str,
    target: Option<&str>,
    details: Option<Json>,
) -> sqlx::Result<()> {
    query(
        "INSERT INTO audit_log (actor_id, actor_email, action, target, details)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(actor.id)
    .bind(&actor.email)
    .bind(action)
    .bind(target)
    .bind(details)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_audit(pool: &PgPool, limit: i64, offset: i64) -> sqlx::Result<Vec<AuditEntry>> {
    query_as(
        "SELECT id, actor_email, action, target, details, created_at
         FROM audit_log ORDER BY id DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn count_audit(pool: &PgPool) -> sqlx::Result<i64> {
    query_as::<_, (i64,)>("SELECT count(*) FROM audit_log")
        .fetch_one(pool)
        .await
        .map(|r| r.0)
}

pub async fn get_setting(pool: &PgPool, key: &str) -> sqlx::Result<Option<String>> {
    let row: Option<(String,)> =
        query_as("SELECT value FROM settings WHERE key = $1").bind(key).fetch_optional(pool).await?;
    Ok(row.map(|r| r.0))
}

pub async fn set_setting(pool: &PgPool, key: &str, value: &str, updated_by: Uuid) -> sqlx::Result<()> {
    query(
        "INSERT INTO settings (key, value, updated_at, updated_by)
         VALUES ($1, $2, now(), $3)
         ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = now(), updated_by = $3",
    )
    .bind(key)
    .bind(value)
    .bind(updated_by)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn count_online_users(pool: &PgPool) -> sqlx::Result<i64> {
    query_as::<_, (i64,)>(
        "SELECT count(*) FROM users WHERE last_seen > now() - interval '2 minutes' AND NOT ban_permanent AND (banned_until IS NULL OR banned_until <= now())",
    )
    .fetch_one(pool)
    .await
    .map(|r| r.0)
}

pub async fn recent_users(pool: &PgPool, limit: i64) -> sqlx::Result<Vec<User>> {
    query_as(
        "SELECT id, google_sub, email, name, picture, role, banned_until, ban_permanent,
                ban_reason, lang, theme, created_at, last_seen
         FROM users ORDER BY created_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}
