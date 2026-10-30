use sqlx::{query, query_as, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use super::user::User;

#[derive(Clone, Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub csrf: String,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    csrf: &str,
    user_agent: Option<&str>,
    ip: Option<&str>,
    expires_at: OffsetDateTime,
) -> sqlx::Result<()> {
    query(
        "INSERT INTO sessions (user_id, token_hash, csrf, user_agent, ip, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(csrf)
    .bind(user_agent)
    .bind(ip)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Tim session con han + user tuong ung (2 query de tranh trung ten cot).
pub async fn session_user(pool: &PgPool, token_hash: &str) -> sqlx::Result<Option<(Session, User)>> {
    let s: Option<Session> = query_as(
        "SELECT id, user_id, csrf, created_at, expires_at
         FROM sessions WHERE token_hash = $1 AND expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    let Some(s) = s else { return Ok(None) };
    let u: Option<User> = query_as(
        "SELECT id, google_sub, email, name, picture, role, banned_until,
                ban_permanent, ban_reason, lang, theme, created_at, last_seen
         FROM users WHERE id = $1",
    )
    .bind(s.user_id)
    .fetch_optional(pool)
    .await?;
    Ok(u.map(|u| (s, u)))
}

pub async fn delete_session(pool: &PgPool, token_hash: &str) -> sqlx::Result<()> {
    query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_user_sessions(pool: &PgPool, user_id: Uuid) -> sqlx::Result<u64> {
    let r = query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn touch_session(pool: &PgPool, id: Uuid) -> sqlx::Result<()> {
    query("UPDATE sessions SET last_seen = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
