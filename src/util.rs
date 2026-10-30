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

/// Thoi gian hien tai theo timezone mac dinh cua app (GMT+7).
pub fn local_now() -> OffsetDateTime {
    OffsetDateTime::now_utc().to_offset(time::macros::offset!(+7))
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
    fn test_truncate() {
        assert_eq!(truncate_chars("abc", 5), "abc");
        assert_eq!(truncate_chars("abcdefgh", 3), "abc…");
    }
}
