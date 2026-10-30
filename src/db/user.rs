use sqlx::{query, query_as, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct User {
    pub id: Uuid,
    pub google_sub: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
    pub role: String,
    pub banned_until: Option<OffsetDateTime>,
    pub ban_permanent: bool,
    pub ban_reason: Option<String>,
    pub created_at: OffsetDateTime,
    pub last_seen: OffsetDateTime,
}

pub fn role_rank(role: &str) -> i8 {
    match role {
        "admin" => 2,
        "moderator" => 1,
        _ => 0,
    }
}

impl User {
    pub fn rank(&self) -> i8 {
        role_rank(&self.role)
    }
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
    pub fn is_staff(&self) -> bool {
        self.rank() >= 1
    }
    pub fn is_banned(&self, now: OffsetDateTime) -> bool {
        self.ban_permanent || self.banned_until.map(|t| t > now).unwrap_or(false)
    }
    pub fn is_online(&self, now: OffsetDateTime) -> bool {
        !self.is_banned(now) && (now - self.last_seen).whole_seconds().abs() < 120
    }
}

pub async fn upsert_google_user(
    pool: &PgPool,
    google_sub: &str,
    email: &str,
    name: &str,
    picture: Option<&str>,
) -> sqlx::Result<User> {
    query_as(
        "INSERT INTO users (google_sub, email, name, picture)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (google_sub) DO UPDATE SET email = $2, name = $3, picture = $4
         RETURNING id, google_sub, email, name, picture, role, banned_until, ban_permanent, ban_reason, created_at, last_seen",
    )
    .bind(google_sub)
    .bind(email)
    .bind(name)
    .bind(picture)
    .fetch_one(pool)
    .await
}

pub async fn get_user(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<User>> {
    query_as(
        "SELECT id, google_sub, email, name, picture, role, banned_until, ban_permanent, ban_reason, created_at, last_seen
         FROM users WHERE id = $1",
    )
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn count_users(pool: &PgPool) -> sqlx::Result<i64> {
    query_as::<_, (i64,)>("SELECT count(*) FROM users")
        .fetch_one(pool)
        .await
        .map(|r| r.0)
}

pub async fn list_users(pool: &PgPool, q: &str, limit: i64, offset: i64) -> sqlx::Result<Vec<User>> {
    let pattern = format!("%{}%", q.trim().to_lowercase());
    query_as(
        "SELECT id, google_sub, email, name, picture, role, banned_until, ban_permanent, ban_reason, created_at, last_seen
         FROM users
         WHERE $1 = '%%' OR lower(name) LIKE $1 OR lower(email) LIKE $1
         ORDER BY last_seen DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(&pattern)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn set_role(pool: &PgPool, id: Uuid, role: &str) -> sqlx::Result<()> {
    query("UPDATE users SET role = $2 WHERE id = $1")
        .bind(id)
        .bind(role)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_ban(
    pool: &PgPool,
    id: Uuid,
    until: Option<OffsetDateTime>,
    permanent: bool,
    reason: Option<&str>,
) -> sqlx::Result<()> {
    query("UPDATE users SET banned_until = $2, ban_permanent = $3, ban_reason = $4 WHERE id = $1")
        .bind(id)
        .bind(until)
        .bind(permanent)
        .bind(reason)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_user(pool: &PgPool, id: Uuid) -> sqlx::Result<()> {
    query("DELETE FROM users WHERE id = $1").bind(id).execute(pool).await?;
    Ok(())
}

/// Cap nhat last_seen, chi ghi khi da cach qua 30s de giam tai viet.
pub async fn touch_last_seen(pool: &PgPool, id: Uuid) -> sqlx::Result<()> {
    query("UPDATE users SET last_seen = now() WHERE id = $1 AND last_seen < now() - interval '30 seconds'")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_rank() {
        assert_eq!(role_rank("user"), 0);
        assert_eq!(role_rank("moderator"), 1);
        assert_eq!(role_rank("admin"), 2);
        assert_eq!(role_rank("khac"), 0);
    }

    #[test]
    fn test_ban_logic() {
        let now = OffsetDateTime::now_utc();
        let base = User {
            id: Uuid::new_v4(),
            google_sub: "s".into(),
            email: "a@b.c".into(),
            name: "A".into(),
            picture: None,
            role: "user".into(),
            banned_until: None,
            ban_permanent: false,
            ban_reason: None,
            created_at: now,
            last_seen: now,
        };
        assert!(!base.clone().is_banned(now));
        let banned = User { banned_until: Some(now + time::Duration::hours(1)), ..base.clone() };
        assert!(banned.is_banned(now));
        let expired = User { banned_until: Some(now - time::Duration::hours(1)), ..base };
        assert!(!expired.is_banned(now));
    }
}
