use serde_json::Value as Json;
use sqlx::{query, query_as, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Conversation {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Message {
    pub id: Uuid,
    pub seq: i64,
    pub role: String,
    pub content: String,
    pub thinking: Option<String>,
    pub tools: Option<Json>,
    pub finish_reason: Option<String>,
    pub duration_ms: Option<i32>,
    pub created_at: OffsetDateTime,
}

pub async fn create_conversation(pool: &PgPool, user_id: Uuid, title: &str) -> sqlx::Result<Conversation> {
    query_as(
        "INSERT INTO conversations (user_id, title) VALUES ($1, $2)
         RETURNING id, user_id, title, created_at, updated_at",
    )
    .bind(user_id)
    .bind(title)
    .fetch_one(pool)
    .await
}

pub async fn list_conversations(pool: &PgPool, user_id: Uuid) -> sqlx::Result<Vec<Conversation>> {
    query_as(
        "SELECT id, user_id, title, created_at, updated_at
         FROM conversations WHERE user_id = $1
         ORDER BY updated_at DESC LIMIT 100",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn get_conversation(pool: &PgPool, id: Uuid, user_id: Uuid) -> sqlx::Result<Option<Conversation>> {
    query_as(
        "SELECT id, user_id, title, created_at, updated_at
         FROM conversations WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

pub async fn rename_conversation(pool: &PgPool, id: Uuid, user_id: Uuid, title: &str) -> sqlx::Result<()> {
    query("UPDATE conversations SET title = $3, updated_at = now() WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .bind(title)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_conversation(pool: &PgPool, id: Uuid, user_id: Uuid) -> sqlx::Result<()> {
    query("DELETE FROM conversations WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn touch_conversation(pool: &PgPool, id: Uuid) -> sqlx::Result<()> {
    query("UPDATE conversations SET updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn count_conversations(pool: &PgPool) -> sqlx::Result<i64> {
    query_as::<_, (i64,)>("SELECT count(*) FROM conversations")
        .fetch_one(pool)
        .await
        .map(|r| r.0)
}

/// Du lieu de them mot tin nhan moi.
pub struct NewMessage<'a> {
    pub role: &'a str,
    pub content: &'a str,
    pub thinking: Option<&'a str>,
    pub tools: Option<&'a Json>,
    pub finish_reason: Option<&'a str>,
    pub duration_ms: Option<i32>,
}

pub async fn insert_message(
    pool: &PgPool,
    conversation_id: Uuid,
    msg: NewMessage<'_>,
) -> sqlx::Result<Message> {
    query_as(
        "INSERT INTO messages (conversation_id, role, content, thinking, tools, finish_reason, duration_ms)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id, seq, role, content, thinking, tools, finish_reason, duration_ms, created_at",
    )
    .bind(conversation_id)
    .bind(msg.role)
    .bind(msg.content)
    .bind(msg.thinking)
    .bind(msg.tools)
    .bind(msg.finish_reason)
    .bind(msg.duration_ms)
    .fetch_one(pool)
    .await
}

pub async fn list_messages(pool: &PgPool, conversation_id: Uuid, limit: i64) -> sqlx::Result<Vec<Message>> {
    query_as(
        "SELECT id, seq, role, content, thinking, tools, finish_reason, duration_ms, created_at
         FROM messages WHERE conversation_id = $1
         ORDER BY seq ASC LIMIT $2",
    )
    .bind(conversation_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Xoa cac tin nhan phia sau tin user cuoi cung (dung cho regenerate).
pub async fn delete_after_last_user(pool: &PgPool, conversation_id: Uuid) -> sqlx::Result<u64> {
    let r = query(
        "DELETE FROM messages
         WHERE conversation_id = $1
           AND seq > COALESCE(
             (SELECT MAX(seq) FROM messages WHERE conversation_id = $1 AND role = 'user'), 0
           )",
    )
    .bind(conversation_id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

pub async fn count_messages_since(pool: &PgPool, since: OffsetDateTime) -> sqlx::Result<i64> {
    query_as::<_, (i64,)>("SELECT count(*) FROM messages WHERE created_at >= $1")
        .bind(since)
        .fetch_one(pool)
        .await
        .map(|r| r.0)
}

pub async fn count_messages(pool: &PgPool) -> sqlx::Result<i64> {
    query_as::<_, (i64,)>("SELECT count(*) FROM messages")
        .fetch_one(pool)
        .await
        .map(|r| r.0)
}

/// So tin nhan moi ngay trong N ngay gan nhat (khong tinh ngay hom nay).
pub async fn messages_per_day(pool: &PgPool, days: i32) -> sqlx::Result<Vec<(String, i64)>> {
    let rows: Vec<(time::Date, i64)> = query_as(
        "SELECT (created_at AT TIME ZONE 'Asia/Ho_Chi_Minh')::date AS d, count(*)
         FROM messages
         WHERE created_at >= now() - make_interval(days => $1)
         GROUP BY 1 ORDER BY 1",
    )
    .bind(days)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(d, c)| {
            (
                format!("{:02}/{:02}", d.day(), d.month() as u8),
                c,
            )
        })
        .collect())
}
