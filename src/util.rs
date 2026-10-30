use pulldown_cmark::{html::push_html, Options, Parser};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;


/// Bien markdown thanh HTML an toan (da sanitize).
pub fn markdown_to_html(md: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(md, opts);
    let mut out = String::new();
    push_html(&mut out, parser);
    ammonia::clean(&out)
}

pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

const VN_DAYS: [&str; 7] = ["Chủ nhật", "Thứ hai", "Thứ ba", "Thứ tư", "Thứ năm", "Thứ sáu", "Thứ bảy"];

/// Thoi gian hien tai theo timezone mac dinh cua app (GMT+7).
pub fn local_now() -> OffsetDateTime {
    OffsetDateTime::now_utc().to_offset(time::macros::offset!(+7))
}

/// "Thu Tu, 30/10/2026 14:30:05 (GMT+7)"
pub fn format_full(t: OffsetDateTime) -> String {
    let weekday = VN_DAYS
        .get(t.weekday().number_days_from_monday() as usize % 7)
        .copied()
        .unwrap_or("");
    let dd = t.day();
    let mm = t.month() as u8;
    format!("{weekday}, {dd:02}/{mm:02}/{} {:02}:{:02}:{:02} (GMT+7)", t.year(), t.hour(), t.minute(), t.second())
}

/// Thoi gian tuong doi tieng Viet.
pub fn relative_time(t: OffsetDateTime, now: OffsetDateTime) -> String {
    let secs = (now - t).whole_seconds().max(0);
    match secs {
        0..=45 => "vừa xong".into(),
        46..=1500 => format!("{} phút trước", ((secs + 59) / 60).max(1)),
        1501..=72_000 => format!("{} giờ trước", ((secs + 3599) / 3600).max(1)),
        _ => {
            let days = (secs + 43_199) / 86_400;
            match days {
                1..=2 => format!("{days} ngày trước"),
                _ => format!("{:02}/{:02}/{}", t.day(), t.month() as u8, t.year()),
            }
        }
    }
}

pub fn to_rfc3339(t: OffsetDateTime) -> String {
    t.format(&Rfc3339).unwrap_or_default()
}

/// Escape HTML thong dung.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Bo sung "..." neu qua dai, cat tai vi tri an toan theo so ky tu.
pub fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    let trimmed = cut.trim_end();
    format!("{trimmed}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    #[test]
    fn test_markdown_basic() {
        let html = markdown_to_html("# Xin chào\n\n**đậm** và `code`");
        assert!(html.contains("<h1>Xin chào</h1>"));
        assert!(html.contains("<strong>đậm</strong>"));
        assert!(html.contains("<code>code</code>"));
    }

    #[test]
    fn test_markdown_strips_script() {
        let html = markdown_to_html("<script>alert(1)</script>hi");
        assert!(!html.contains("<script"));
    }

    #[test]
    fn test_relative_time() {
        let now = OffsetDateTime::now_utc();
        assert_eq!(relative_time(now, now), "vừa xong");
        assert_eq!(relative_time(now - Duration::minutes(5), now), "5 phút trước");
        assert_eq!(relative_time(now - Duration::hours(3), now), "3 giờ trước");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate_chars("abc", 5), "abc");
        assert_eq!(truncate_chars("abcdefgh", 3), "abc…");
    }
}
