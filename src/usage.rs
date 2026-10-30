//! Mức dùng hàng giờ: 200 tin nhắn / giờ, hết hạn mức chờ 1h05 để mở lại.
//! Lỗi 429 từ gateway chỉ ép khoá một lần, các lần thử sau không cộng dồn thời gian chờ.

use serde_json::{json, Value as Json};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

pub const HOURLY_LIMIT: i32 = 200;
/// 1 giờ + 5 phút đệm.
pub const COOLDOWN_SECS: i64 = 65 * 60;
const WINDOW_SECS: i64 = 3600;

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub used: i32,
    pub limit: i32,
    /// Đang trong thời gian chờ mở lại hạn mức.
    pub blocked: bool,
    /// Thời điểm hạn mức mở lại (khi bị khoá) hoặc cửa sổ hiện tại kết thúc.
    pub resets_at: Option<OffsetDateTime>,
}

impl Snapshot {
    pub fn percent(&self) -> i32 {
        if self.limit <= 0 {
            return 0;
        }
        (((self.used as f64 / self.limit as f64) * 100.0).round() as i32).min(100)
    }

    /// Thời điểm mở lại hạn mức dạng RFC 3339 (rỗng nếu không có).
    pub fn resets_at_rfc3339(&self) -> String {
        self.resets_at.map(crate::util::to_rfc3339).unwrap_or_default()
    }

    pub fn to_json(&self) -> Json {
        json!({
            "used": self.used,
            "limit": self.limit,
            "percent": self.percent(),
            "blocked": self.blocked,
            "resets_at": self.resets_at.map(crate::util::to_rfc3339),
            "resets_in": self.resets_at
                .map(|t| (t - OffsetDateTime::now_utc()).whole_seconds().max(0)),
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct Row {
    window_start: Option<OffsetDateTime>,
    used: i32,
    blocked_until: Option<OffsetDateTime>,
}

fn to_snapshot(r: Row) -> Snapshot {
    let now = OffsetDateTime::now_utc();
    let blocked = r.blocked_until.map(|t| t > now).unwrap_or(false);
    let resets_at = if blocked {
        r.blocked_until
    } else {
        r.window_start.map(|w| w + time::Duration::seconds(WINDOW_SECS))
    };
    Snapshot {
        used: r.used.clamp(0, HOURLY_LIMIT),
        limit: HOURLY_LIMIT,
        blocked,
        resets_at: resets_at.filter(|t| *t > now),
    }
}

pub enum Decision {
    Allow(Snapshot),
    Deny(Snapshot),
}

/// Đọc trạng thái hiện tại mà không ghi nhận thêm.
pub async fn snapshot(pool: &PgPool, user_id: Uuid) -> Snapshot {
    let row = sqlx::query_as::<_, (Option<OffsetDateTime>, i32, Option<OffsetDateTime>)>(
        "SELECT window_start, used, blocked_until FROM usage_state WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|(window_start, used, blocked_until)| Row { window_start, used, blocked_until });
    row.map(to_snapshot)
        .unwrap_or_else(|| Snapshot {
            used: 0,
            limit: HOURLY_LIMIT,
            blocked: false,
            resets_at: None,
        })
}

/// Kiểm tra và ghi nhận một lượt chat trong một UPDATE nguyên tố.
///
/// Lượt thứ 200 vẫn được thực hiện; từ lượt sau bị chờ 65 phút. Khi hết chờ,
/// cửa sổ cũ đã quá hạn nên bộ đếm tự bắt đầu lại từ 1.
pub async fn check_and_record(pool: &PgPool, user_id: Uuid) -> Decision {
    let _ = sqlx::query("INSERT INTO usage_state (user_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(user_id)
        .execute(pool)
        .await;

    let res = sqlx::query_as::<_, (Option<OffsetDateTime>, i32, Option<OffsetDateTime>, bool)>(
        r#"
        WITH prev AS (
            SELECT window_start AS p_ws, used AS p_used, blocked_until AS p_bu
            FROM usage_state WHERE user_id = $1
            FOR UPDATE
        )
        UPDATE usage_state s SET
            used = CASE
                WHEN prev.p_bu IS NOT NULL AND prev.p_bu > now() THEN s.used
                WHEN now() - prev.p_ws >= interval '1 hour' THEN 1
                ELSE prev.p_used + 1
            END,
            window_start = CASE
                WHEN prev.p_bu IS NOT NULL AND prev.p_bu > now() THEN s.window_start
                WHEN now() - prev.p_ws >= interval '1 hour' THEN now()
                ELSE s.window_start
            END,
            blocked_until = CASE
                WHEN prev.p_bu IS NOT NULL AND prev.p_bu > now() THEN s.blocked_until
                WHEN (CASE
                        WHEN now() - prev.p_ws >= interval '1 hour' THEN 1
                        ELSE prev.p_used + 1
                     END) >= $2
                    THEN now() + make_interval(secs => $3)
                ELSE NULL
            END,
            updated_at = now()
        FROM prev
        WHERE s.user_id = $1
        RETURNING s.window_start, s.used, s.blocked_until,
                  (prev.p_bu IS NOT NULL AND prev.p_bu > now()) AS was_blocked
        "#,
    )
    .bind(user_id)
    .bind(HOURLY_LIMIT)
    .bind(COOLDOWN_SECS)
    .fetch_one(pool)
    .await;

    match res {
        Ok((window_start, used, blocked_until, was_blocked)) => {
            let snap = to_snapshot(Row { window_start, used, blocked_until });
            if was_blocked {
                Decision::Deny(snap)
            } else {
                Decision::Allow(snap)
            }
        }
        Err(e) => {
            tracing::error!("ghi usage loi: {e}");
            // Lỗi bộ đếm không nên chặn người dùng chat
            Decision::Allow(Snapshot {
                used: 0,
                limit: HOURLY_LIMIT,
                blocked: false,
                resets_at: None,
            })
        }
    }
}

/// Gateway trả 429: ép thanh mức dùng về 100% và khoá 65 phút.
/// Chỉ có hiệu lực khi chưa bị khoá — người dùng thử đi thử lại không cộng dồn.
pub async fn force_block(pool: &PgPool, user_id: Uuid) -> Snapshot {
    let _ = sqlx::query("INSERT INTO usage_state (user_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(user_id)
        .execute(pool)
        .await;

    let res = sqlx::query_as::<_, (Option<OffsetDateTime>, i32, Option<OffsetDateTime>)>(
        r#"
        UPDATE usage_state SET
            used = CASE
                WHEN blocked_until IS NOT NULL AND blocked_until > now() THEN used
                ELSE $2
            END,
            blocked_until = CASE
                WHEN blocked_until IS NOT NULL AND blocked_until > now() THEN blocked_until
                ELSE now() + make_interval(secs => $3)
            END,
            window_start = CASE
                WHEN blocked_until IS NOT NULL AND blocked_until > now() THEN window_start
                ELSE now()
            END,
            updated_at = now()
        WHERE user_id = $1
        RETURNING window_start, used, blocked_until
        "#,
    )
    .bind(user_id)
    .bind(HOURLY_LIMIT)
    .bind(COOLDOWN_SECS)
    .fetch_one(pool)
    .await;

    match res {
        Ok((window_start, used, blocked_until)) => {
            to_snapshot(Row { window_start, used, blocked_until })
        }
        Err(e) => {
            tracing::error!("force block loi: {e}");
            snapshot(pool, user_id).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percent() {
        let s = Snapshot {
            used: 100,
            limit: 200,
            blocked: false,
            resets_at: None,
        };
        assert_eq!(s.percent(), 50);
        let s = Snapshot { used: 200, ..s };
        assert_eq!(s.percent(), 100);
    }

    #[test]
    fn test_percent_clamps() {
        let s = Snapshot {
            used: 250,
            limit: 200,
            blocked: false,
            resets_at: None,
        };
        assert_eq!(s.percent(), 100);
    }
}
