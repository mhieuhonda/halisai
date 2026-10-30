//! Công cụ cho agent: web_search, web_fetch, datetime.
//!
//! Kết quả trả về cho model dùng tiếng Anh (chuẩn mô tả công cụ), phần tóm tắt
//! hiển thị trên giao diện được bản địa hoá theo ngôn ngữ người dùng.

use serde_json::{json, Value as Json};
use time::OffsetDateTime;
use time_tz::{Offset as _, TimeZone as _};

const SEARCH_MAX_QUERIES: usize = 3;
const SEARCH_MAX_RESULTS: usize = 6;
const SEARCH_TOTAL_RESULTS: usize = 12;
const SEARCH_SNIPPET_CHARS: usize = 300;
const QUERY_MAX_CHARS: usize = 400;

const FETCH_MAX_BYTES: usize = 2 * 1024 * 1024;
const FETCH_OUTPUT_CHARS: usize = 16_000;

pub struct ToolResult {
    pub ok: bool,
    pub output: String,
    pub summary: String,
}

impl ToolResult {
    fn ok(output: String, summary: String) -> Self {
        Self { ok: true, output, summary }
    }

    fn err(output: String) -> Self {
        Self { summary: output.clone(), output, ok: false }
    }
}

/// Các HTTP client dùng riêng cho công cụ, tạo một lần khi khởi động.
pub struct Tools {
    fetch: reqwest::Client,
    search: reqwest::Client,
}

impl Tools {
    pub fn new() -> Self {
        let fetch = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(8))
            .timeout(std::time::Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::limited(4))
            .user_agent(DESKTOP_UA)
            .build()
            .expect("tao fetch client");
        let search = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(6))
            .timeout(std::time::Duration::from_secs(12))
            .redirect(reqwest::redirect::Policy::limited(3))
            .user_agent(DESKTOP_UA)
            .build()
            .expect("tao search client");
        Self { fetch, search }
    }

    pub async fn execute(&self, name: &str, args: &Json, lang: &str) -> ToolResult {
        match name {
            "web_search" => web_search(self, args, lang).await,
            "web_fetch" => web_fetch(self, args, lang).await,
            "datetime" => datetime(args, lang),
            _ => ToolResult::err(format!("Unknown tool: {name}")),
        }
    }
}

/// Dinh nghia cac cong cu trinh bay cho model.
pub fn schemas() -> Json {
    json!([
        {
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the web for up-to-date information. Use for news, prices, releases, documentation, events, or any fact that may have changed over time. Prefer short keyword-style queries; send up to 3 queries in one call to cover different angles.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "queries": {
                            "type": "array",
                            "items": { "type": "string" },
                            "minItems": 1,
                            "maxItems": 3,
                            "description": "1-3 search queries. Each query is a short phrase of keywords, not a full sentence."
                        }
                    },
                    "required": ["queries"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "web_fetch",
                "description": "Fetch a public web page and return its content converted to markdown. Use when the user gives a specific URL, or to read a page in full after web_search found it.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "Full http(s) URL to read, including the scheme."
                        }
                    },
                    "required": ["url"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "datetime",
                "description": "Get the current date and time. Returns the full date, weekday, time, ISO 8601 timestamp and Unix epoch for a given timezone. Use it whenever the answer depends on the current moment (deadlines, schedules, relative dates).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "timezone": {
                            "type": "string",
                            "description": "Optional timezone: an IANA name such as Asia/Ho_Chi_Minh or America/New_York, or a UTC offset such as +7 or -05:30. Defaults to Asia/Ho_Chi_Minh (GMT+7)."
                        }
                    },
                    "required": []
                }
            }
        }
    ])
}

// =====================================================================
// datetime
// =====================================================================

/// Bảng viết tắt múi giờ thường gặp.
const TZ_ALIASES: &[(&str, &str)] = &[
    ("UTC", "+0"), ("GMT", "+0"), ("Z", "+0"),
    ("ICT", "+7"), ("WIB", "+7"), ("WITA", "+8"), ("WIT", "+9"),
    ("JST", "+9"), ("KST", "+9"),
    ("CET", "+1"), ("CEST", "+2"), ("BST", "+1"), ("WET", "+0"), ("WEST", "+1"),
    ("EET", "+2"), ("EEST", "+3"), ("MSK", "+3"),
    ("EST", "-5"), ("EDT", "-4"), ("CST", "-6"), ("CDT", "-5"),
    ("MST", "-7"), ("MDT", "-6"), ("PST", "-8"), ("PDT", "-7"),
    ("AKST", "-9"), ("AKDT", "-8"), ("HST", "-10"),
    ("IST", "+5:30"), ("NPT", "+5:45"), ("BST+1", "+1"),
    ("AEST", "+10"), ("AEDT", "+11"), ("ACST", "+9:30"), ("NZST", "+12"), ("NZDT", "+13"),
    ("BRST", "-3"), ("BRT", "-3"), ("ART", "-3"), ("CLT", "-4"), ("PET", "-5"), ("COT", "-5"),
];

/// Giải chuỗi múi giờ thành offset. Hỗ trợ tên IANA, viết tắt phổ biến và offset dạng +7 / -05:30.
fn resolve_timezone(input: &str) -> Result<time::UtcOffset, String> {
    let raw = input.trim();
    if raw.is_empty() {
        return Ok(time::macros::offset!(+7));
    }
    let cleaned = raw.replace(' ', "_");
    if let Some(tz) = time_tz::timezones::get_by_name(&cleaned) {
        let base = OffsetDateTime::now_utc();
        return Ok(tz.get_offset_utc(&base).to_utc());
    }
    let upper = raw.to_uppercase();
    if let Some((_, off)) = TZ_ALIASES.iter().find(|(k, _)| *k == upper) {
        return parse_offset(off);
    }
    // Dạng +7, -05:30, gmt+7, utc+09:00
    let stripped = upper
        .trim_start_matches("UTC")
        .trim_start_matches("GMT")
        .trim_start_matches("GIỜ")
        .trim();
    parse_offset(stripped)
}

fn parse_offset(s: &str) -> Result<time::UtcOffset, String> {
    let s = s.trim();
    let (sign, rest) = if let Some(r) = s.strip_prefix('+') {
        (1, r)
    } else if let Some(r) = s.strip_prefix('-') {
        (-1, r)
    } else {
        return Err(format!("unsupported timezone: {s}"));
    };
    let rest = rest.trim();
    let (h_str, m_str) = match rest.split_once(':') {
        Some((h, m)) => (h, m),
        None => (rest, "0"),
    };
    let h: i8 = h_str.trim().parse().map_err(|_| format!("unsupported timezone: {s}"))?;
    let m: i8 = m_str.trim().parse().map_err(|_| format!("unsupported timezone: {s}"))?;
    if h < 0 || !(0..60).contains(&m) || h > 14 {
        return Err(format!("timezone out of range: {s}"));
    }
    time::UtcOffset::from_hms(sign * h, sign * m, 0).map_err(|_| format!("unsupported timezone: {s}"))
}

const DAYS_EN: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MONTHS_EN: [&str; 12] = [
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
];

fn weekday_en(t: OffsetDateTime) -> &'static str {
    DAYS_EN[(t.weekday().number_days_from_monday() as usize) % 7]
}

fn fmt_offset(off: time::UtcOffset) -> String {
    let sign = if off.is_negative() { '-' } else { '+' };
    let abs = off.whole_hours().abs();
    let mins = off.minutes_past_hour().abs();
    format!("{sign}{abs:02}:{mins:02}")
}

/// Dạng ngắn cho tóm tắt: +7, +9, -5 hoặc +5:30 khi có phút lẻ.
fn fmt_offset_short(off: time::UtcOffset) -> String {
    let sign = if off.is_negative() { '-' } else { '+' };
    let h = off.whole_hours().abs();
    let m = off.minutes_past_hour().abs();
    if m == 0 {
        format!("{sign}{h}")
    } else {
        format!("{sign}{h}:{m:02}")
    }
}

fn datetime(args: &Json, lang: &str) -> ToolResult {
    let tz_input = args.get("timezone").and_then(|v| v.as_str()).unwrap_or("");
    let offset = match resolve_timezone(tz_input) {
        Ok(o) => o,
        Err(_) => {
            return ToolResult::err(
                "Unrecognized timezone. Use an IANA name (e.g. Asia/Ho_Chi_Minh, America/New_York, Europe/London) or a UTC offset such as +7 or -05:30.".into(),
            )
        }
    };

    let now = OffsetDateTime::now_utc().to_offset(offset);
    let utc = OffsetDateTime::now_utc();
    let iso = now.format(&time::format_description::well_known::Rfc3339).unwrap_or_default();
    let iso_utc = utc.format(&time::format_description::well_known::Rfc3339).unwrap_or_default();
    let month_en = MONTHS_EN[(now.month() as usize - 1) % 12];
    let week = now.iso_week();

    let output = format!(
        "Current date and time ({}, UTC{}):\n\
         - Full: {}, {} {} {}, {:02}:{:02}:{:02}\n\
         - Date: {:04}-{:02}-{:02} ({})\n\
         - Time: {:02}:{:02}:{:02}\n\
         - ISO 8601: {}\n\
         - Unix timestamp: {}\n\
         - ISO week: {}-W{:02}\n\
         Reference (UTC): {}\n\
         This is the authoritative current time. Use it for anything time-sensitive.",
        tz_label(tz_input, offset),
        fmt_offset(offset),
        weekday_en(now),
        now.day(),
        month_en,
        now.year(),
        now.hour(),
        now.minute(),
        now.second(),
        now.year(),
        now.month() as u8,
        now.day(),
        weekday_en(now),
        now.hour(),
        now.minute(),
        now.second(),
        iso,
        now.unix_timestamp(),
        now.year(),
        week,
        iso_utc,
    );

    // Tóm tắt bản địa hoá cho giao diện
    let summary = if lang == "en" {
        format!("{} {} {}, {} · {} UTC{}", weekday_en(now), month_en, now.day(), now.year(), fmt_clock(now, false), fmt_offset(offset))
    } else {
        format!("{}/{:02}/{:04} · {} GMT{}", now.day(), now.month() as u8, now.year(), fmt_clock(now, true), fmt_offset_short(offset))
    };
    ToolResult::ok(output, summary)
}

fn fmt_clock(t: OffsetDateTime, h24: bool) -> String {
    if h24 {
        format!("{:02}:{:02}", t.hour(), t.minute())
    } else {
        let (h, ampm) = if t.hour() < 12 {
            (if t.hour() == 0 { 12 } else { t.hour() }, "AM")
        } else {
            (if t.hour() == 12 { 12 } else { t.hour() - 12 }, "PM")
        };
        format!("{h}:{:02} {}", t.minute(), ampm)
    }
}

/// Nhãn múi giờ: giữ nguyên tên IANA nếu có, ngược lại mô tả offset.
fn tz_label(input: &str, offset: time::UtcOffset) -> String {
    let cleaned = input.trim().replace(' ', "_");
    if !cleaned.is_empty()
        && !cleaned.starts_with('+')
        && !cleaned.starts_with('-')
        && time_tz::timezones::get_by_name(&cleaned).is_some()
    {
        return cleaned;
    }
    format!("UTC{}", fmt_offset(offset))
}

// =====================================================================
// web_search
// =====================================================================

const DESKTOP_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Clone, Debug, Default)]
struct SearchHit {
    title: String,
    url: String,
    snippet: String,
}

async fn web_search(tools: &Tools, args: &Json, lang: &str) -> ToolResult {
    let mut queries: Vec<String> = match args.get("queries") {
        Some(Json::Array(list)) => list
            .iter()
            .filter_map(|q| q.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        // Một số model gửi "query" thay vì "queries" — chấp nhận cả hai
        Some(Json::String(s)) if !s.trim().is_empty() => vec![s.trim().to_string()],
        _ => {
            return ToolResult::err(
                "Missing `queries` (array of 1-3 short keyword queries).".into(),
            )
        }
    };
    queries.dedup();
    if queries.is_empty() {
        return ToolResult::err("Provide at least one non-empty query.".into());
    }
    if queries.len() > SEARCH_MAX_QUERIES {
        queries.truncate(SEARCH_MAX_QUERIES);
    }
    for q in &queries {
        if q.chars().count() > QUERY_MAX_CHARS {
            return ToolResult::err(format!("Query too long (max {QUERY_MAX_CHARS} chars)."));
        }
    }

    // Chạy song song các truy vấn
    let futs: Vec<_> = queries
        .iter()
        .map(|q| {
            let client = tools.search.clone();
            let q = q.clone();
            async move { search_one(&client, &q).await }
        })
        .collect();
    let results = futures::future::join_all(futs).await;
    let total: usize = results.iter().map(|h| h.len()).sum();

    let mut hits: Vec<SearchHit> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut any_error = true;
    for (i, (q, hits_q)) in queries.iter().zip(results).enumerate() {
        if !hits_q.is_empty() {
            any_error = false;
        }
        if i > 0 {
            lines.push(String::new());
        }
        lines.push(format!("Query: \"{q}\""));
        if hits_q.is_empty() {
            lines.push("No results found. Try different keywords.".into());
            continue;
        }
        for h in &hits_q {
            lines.push(format!("- [{}]({}){}", h.title, h.url, snippet_suffix(&h.snippet)));
            hits.push(h.clone());
        }
    }
    hits.truncate(SEARCH_TOTAL_RESULTS);

    let mut output = String::from("Web search results.\n\n");
    output.push_str(&lines.join("\n"));
    output.push_str("\n\nCite the relevant URLs above as markdown links in your answer.");
    if hits.len() >= SEARCH_TOTAL_RESULTS {
        output.push_str(" (Showing the first sources; refine the query for more.)");
    }

    let summary = if lang == "en" {
        match (queries.len(), total) {
            (1, n) => format!("Search \"{}\" · {} result{}", queries[0], n, if n == 1 { "" } else { "s" }),
            (c, n) => format!("Search {c} queries · {} results", n),
        }
    } else {
        match (queries.len(), total) {
            (1, n) => format!("Tìm kiếm \"{}\" · {} kết quả", queries[0], n),
            (c, n) => format!("Tìm kiếm {c} truy vấn · {} kết quả", n),
        }
    };

    if total == 0 && any_error {
        return ToolResult::err("Search is temporarily unavailable. Tell the user search failed and answer from your own knowledge with a note that it may be outdated.".into());
    }
    ToolResult::ok(output, summary)
}

fn snippet_suffix(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        String::new()
    } else {
        format!(" — {s}")
    }
}

/// Tìm trên một truy vấn: thử lần lượt các nguồn cho tới khi có kết quả.
async fn search_one(client: &reqwest::Client, query: &str) -> Vec<SearchHit> {
    // 1. DuckDuckGo bản HTML
    let ddg_html = format!("https://html.duckduckgo.com/html/?q={}", urlencode(query));
    if let Ok(resp) = client.get(&ddg_html).header("accept", "text/html").send().await {
        if resp.status().is_success() {
            if let Ok(body) = resp.bytes().await {
                let hits = parse_ddg_results(&String::from_utf8_lossy(&body));
                if !hits.is_empty() {
                    return hits;
                }
            }
        }
    }
    // 2. DuckDuckGo bản lite
    let ddg_lite = format!("https://lite.duckduckgo.com/lite/?q={}", urlencode(query));
    if let Ok(resp) = client.get(&ddg_lite).header("accept", "text/html").send().await {
        if resp.status().is_success() {
            if let Ok(body) = resp.bytes().await {
                let hits = parse_ddg_results(&String::from_utf8_lossy(&body));
                if !hits.is_empty() {
                    return hits;
                }
            }
        }
    }
    // 3. Bing RSS — XML tĩnh, không cần JS
    let bing_rss = format!("https://www.bing.com/search?q={}&format=rss", urlencode(query));
    if let Ok(resp) = client.get(&bing_rss).header("accept", "application/rss+xml, application/xml").send().await {
        if resp.status().is_success() {
            if let Ok(body) = resp.bytes().await {
                let hits = parse_bing_rss(&String::from_utf8_lossy(&body));
                if !hits.is_empty() {
                    return hits;
                }
            }
        }
    }
    Vec::new()
}

/// Đọc kết quả từ nguồn Bing RSS: <item><title/><link/><description/></item>
fn parse_bing_rss(xml: &str) -> Vec<SearchHit> {
    let mut hits: Vec<SearchHit> = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<item>") {
        let after = &rest[start + 6..];
        let Some(end) = after.find("</item>") else { break };
        let item = &after[..end];
        let title = tag_text(item, "title").unwrap_or_default();
        let url = tag_text(item, "link").unwrap_or_default();
        let snippet = tag_text(item, "description").unwrap_or_default();
        if (url.starts_with("http://") || url.starts_with("https://"))
            && !url.contains("bing.com/")
            && !title.is_empty()
        {
            hits.push(SearchHit {
                title: crate::util::truncate_chars(&collapse_ws(&title), 160),
                url,
                snippet: crate::util::truncate_chars(&collapse_ws(&snippet), SEARCH_SNIPPET_CHARS),
            });
        }
        rest = &after[end + 7..];
        if hits.len() >= SEARCH_MAX_RESULTS {
            break;
        }
    }
    hits
}

/// Lấy văn bản bên trong thẻ XML đơn giản (không lồng nhau).
fn tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    // Bỏ qua thẻ có thuộc tính (dạng <tag ...>)
    let start = xml.find(&open)?;
    let inner = &xml[start + open.len()..];
    let end = inner.find(&close)?;
    Some(decode_entities(&inner[..end]))
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 3 <= bytes.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Thẻ class có chứa token cần tìm (class="result__a ..." ).
fn class_has(class: &str, token: &str) -> bool {
    class.split_whitespace().any(|c| c == token)
}

/// Tách giá trị thuộc tính (đã giải mã entity) từ chuỗi thẻ.
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let needle = format!("{name}=");
    let pos = lower.find(&needle)?;
    let rest = &tag[pos + needle.len()..];
    let mut chars = rest.chars();
    match chars.next()? {
        '"' => {
            let end = rest[1..].find('"')? + 1;
            Some(decode_entities(&rest[1..end]))
        }
        '\'' => {
            let end = rest[1..].find('\'')? + 1;
            Some(decode_entities(&rest[1..end]))
        }
        _ => {
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '>')
                .unwrap_or(rest.len());
            Some(decode_entities(&rest[..end]))
        }
    }
}

/// Thu thập nội dung văn bản đến thẻ đóng tương ứng.
fn collect_text_until(html: &str, start: usize, close_tag: &str) -> (String, usize) {
    let lower = html.to_lowercase();
    let mut out = String::new();
    let mut i = start;
    while i < html.len() {
        let Some(rel) = lower[i..].find('<') else { break };
        let tag_at = i + rel;
        // Văn bản trước thẻ
        if let Some(text) = html.get(i..tag_at) {
            out.push_str(&decode_entities(text));
        }
        if lower[tag_at..].starts_with(&format!("</{close_tag}")) {
            let after = lower[tag_at..].find('>').map(|g| tag_at + g + 1).unwrap_or(html.len());
            return (collapse_ws(&out), after);
        }
        // Bỏ qua thẻ mở con (không lấy marker, chỉ cần text)
        match lower[tag_at..].find('>') {
            Some(g) => i = tag_at + g + 1,
            None => break,
        }
    }
    (collapse_ws(&out), html.len())
}

/// Phân tích trang kết quả tìm kiếm.
fn parse_ddg_results(html: &str) -> Vec<SearchHit> {
    let lower = html.to_lowercase();
    let mut hits: Vec<SearchHit> = Vec::new();
    let mut i = 0usize;
    while let Some(rel) = lower[i..].find("<a ") {
        let tag_at = i + rel;
        let Some(gt) = lower[tag_at..].find('>') else { break };
        let tag_end = tag_at + gt;
        let tag = &html[tag_at..tag_end];
        let class = attr_value(tag, "class").unwrap_or_default();
        let is_link = class_has(&class, "result__a") || class_has(&class, "result-link");
        let is_snippet = class_has(&class, "result__snippet") || class_has(&class, "result-snippet");
        if is_link || is_snippet {
            let href = attr_value(tag, "href").unwrap_or_default();
            let (text, after) = collect_text_until(html, tag_end + 1, "a");
            if is_link {
                if let Some(url) = resolve_ddg_href(&href) {
                    if !url.contains("duckduckgo.com/y.js") && !url.contains("duckduckgo.com/ad") {
                        if hits.len() < SEARCH_MAX_RESULTS {
                            hits.push(SearchHit {
                                title: crate::util::truncate_chars(&text, 160),
                                url,
                                snippet: String::new(),
                            });
                        }
                    }
                }
            } else if let Some(last) = hits.last_mut() {
                if last.snippet.is_empty() {
                    last.snippet = crate::util::truncate_chars(&text, SEARCH_SNIPPET_CHARS);
                }
            }
            i = after.max(tag_end + 1);
        } else {
            i = tag_end + 1;
        }
    }
    hits.retain(|h| !h.url.is_empty() && !h.title.is_empty());
    hits
}

/// Đường dẫn trả về từ trang kết quả thường là redirect //duckduckgo.com/l/?uddg=<url>.
fn resolve_ddg_href(href: &str) -> Option<String> {
    let h = decode_entities(href);
    if let Some(pos) = h.find("uddg=") {
        let rest = &h[pos + 5..];
        let end = rest.find('&').unwrap_or(rest.len());
        let url = percent_decode(&rest[..end]);
        if url.starts_with("http://") || url.starts_with("https://") {
            return Some(url);
        }
        return None;
    }
    if h.starts_with("https://") || h.starts_with("http://") {
        if h.contains("duckduckgo.com/?q") {
            return None;
        }
        return Some(h);
    }
    None
}

// =====================================================================
// web_fetch
// =====================================================================

async fn web_fetch(tools: &Tools, args: &Json, lang: &str) -> ToolResult {
    let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("").trim().to_string();
    if url.is_empty() {
        return ToolResult::err("Missing `url`.".into());
    }
    if url.len() > 2000 {
        return ToolResult::err("URL too long.".into());
    }
    let Ok(parsed) = reqwest::Url::parse(&url) else {
        return ToolResult::err("Invalid URL.".into());
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return ToolResult::err("Only http/https is supported.".into());
    }
    let Some(host) = parsed.host_str() else {
        return ToolResult::err("URL has no host.".into());
    };
    if host_is_private(host) {
        return ToolResult::err("Refusing to fetch private network addresses.".into());
    }

    let resp = match tools.fetch.get(parsed.clone()).send().await {
        Ok(r) => r,
        Err(_) => return ToolResult::err("Could not reach the site (timeout or connection failed).".into()),
    };
    let status = resp.status();
    if !status.is_success() {
        return ToolResult::err(format!("The site responded with HTTP {status}."));
    }
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    let ct_ok = content_type.starts_with("text/")
        || content_type.contains("json")
        || content_type.contains("xml")
        || content_type.contains("javascript")
        || content_type.contains("yaml");
    if !ct_ok {
        return ToolResult::err(format!("Unsupported content type: {content_type}"));
    }
    let Ok(bytes) = resp.bytes().await else {
        return ToolResult::err("Could not read the response body.".into());
    };
    let limited = &bytes[..bytes.len().min(FETCH_MAX_BYTES)];
    let text = String::from_utf8_lossy(limited);
    let is_html = content_type.contains("html");
    let content = if is_html {
        html_to_markdown(&text)
    } else {
        collapse_ws(&text)
    };
    let content = content.trim().to_string();
    if content.is_empty() {
        return ToolResult::err("The page has no readable text content.".into());
    }

    let (body, truncated) = if content.chars().count() > FETCH_OUTPUT_CHARS {
        (crate::util::truncate_chars(&content, FETCH_OUTPUT_CHARS), true)
    } else {
        (content.clone(), false)
    };

    let mut output = format!("URL: {url}\nHTTP {status} · {content_type} · {host}\n\n{body}");
    if truncated {
        output.push_str("\n\n[Page truncated to keep the response short.]");
    }

    let kchars = content.chars().count();
    let summary = if lang == "en" {
        format!("{host} · {} chars{}", format_thousands(kchars), if truncated { " (truncated)" } else { "" })
    } else {
        format!("{host} · {} ký tự{}", format_thousands(kchars), if truncated { " (cắt ngắn)" } else { "" })
    };
    ToolResult::ok(output, summary)
}

fn format_thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn host_is_private(host: &str) -> bool {
    let h = host.trim_end_matches('.').to_lowercase();
    if h == "localhost" || h.ends_with(".local") || h.ends_with(".internal") || h.ends_with(".lan") {
        return true;
    }
    let parts: Vec<&str> = h.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) {
        let o: Vec<u8> = parts.iter().map(|p| p.parse().unwrap()).collect();
        return o[0] == 0
            || o[0] == 10
            || o[0] == 127
            || (o[0] == 172 && (16..=31).contains(&o[1]))
            || (o[0] == 192 && o[1] == 168)
            || (o[0] == 169 && o[1] == 254);
    }
    if h.contains(':') && (h.starts_with("fc") || h.starts_with("fd") || h.starts_with("fe80") || h == "::1") {
        return true;
    }
    false
}

// =====================================================================
// HTML -> Markdown
// =====================================================================

/// Thẻ chứa không hiển thị được: bỏ cả nội dung.
const DROP_TAGS: &[&str] = &[
    "script", "style", "noscript", "template", "head", "title", "iframe", "frame", "frameset",
    "svg", "canvas", "object", "embed", "applet", "select", "option", "datalist", "button",
    "input", "textarea", "nav", "aside", "footer", "audio", "video", "dialog", "label",
];

const RAW_TEXT_TAGS: &[&str] = &["script", "style", "noscript", "template", "textarea"];

#[derive(Clone, Copy, PartialEq)]
enum ListKind {
    Ul,
    Ol(usize),
}

struct Converter {
    out: String,
    /// Thẻ đang bị bỏ (ẩn hoặc thuộc DROP_TAGS), đóng lại khi gặp thẻ đóng khớp.
    skip_stack: Vec<String>,
    list_stack: Vec<ListKind>,
    quote_depth: usize,
    heading: u8,
    in_pre: bool,
    pre_lang: Option<String>,
    pre_buf: String,
    /// Đang thu thập ô bảng thay vì ghi thẳng ra out.
    cell: Option<String>,
    table_rows: Vec<Vec<String>>,
    table_done_header: bool,
    pending_row: Vec<String>,
    cell_is_header: bool,
    link_href: Option<String>,
    list_item_open: bool,
}

pub fn html_to_markdown(html: &str) -> String {
    let mut c = Converter {
        out: String::with_capacity(html.len() / 2 + 64),
        skip_stack: Vec::new(),
        list_stack: Vec::new(),
        quote_depth: 0,
        heading: 0,
        in_pre: false,
        pre_lang: None,
        pre_buf: String::new(),
        cell: None,
        table_rows: Vec::new(),
        table_done_header: false,
        pending_row: Vec::new(),
        cell_is_header: false,
        link_href: None,
        list_item_open: false,
    };
    c.run(html);
    // Dọn whitespace thừa
    let mut text = std::mem::take(&mut c.out);
    text = text.replace('\r', "");
    // Không quá 2 dòng trống liên tiếp
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    text.trim().to_string()
}

impl Converter {
    fn run(&mut self, html: &str) {
        let lower = html.to_lowercase();
        let bytes = html.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] != b'<' {
                let next = lower[i..].find('<').map(|p| i + p).unwrap_or(html.len());
                let text = html.get(i..next).unwrap_or("");
                self.text(text);
                i = next;
                continue;
            }
            // Comment
            if lower[i..].starts_with("<!--") {
                match lower[i..].find("-->") {
                    Some(end) => {
                        i += end + 3;
                        continue;
                    }
                    None => break,
                }
            }
            // Doctype / processing
            if lower[i..].starts_with("<!") || lower[i..].starts_with("<?") {
                match lower[i..].find('>') {
                    Some(end) => {
                        i += end + 1;
                        continue;
                    }
                    None => break,
                }
            }
            let closing = lower[i..].starts_with("</");
            let name_start = if closing { i + 2 } else { i + 1 };
            let name_end = html[name_start..]
                .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
                .map(|p| name_start + p)
                .unwrap_or(html.len());
            if name_start >= name_end {
                i += 1;
                continue;
            }
            let name = lower[name_start..name_end].trim().to_string();
            let tag_close = match lower[name_end..].find('>') {
                Some(g) => name_end + g,
                None => break,
            };
            let self_closing = html[name_end..tag_close].trim_end().ends_with('/');
            let tag_src = &html[name_start..tag_close];

            if closing {
                self.close_tag(&name);
                i = tag_close + 1;
                continue;
            }

            // Thẻ raw-text: bỏ thẳng tới thẻ đóng
            if RAW_TEXT_TAGS.contains(&name.as_str()) && !self_closing {
                let close = format!("</{name}");
                match lower[tag_close + 1..].find(&close) {
                    Some(rel) => {
                        let after = tag_close + 1 + rel;
                        i = match lower[after..].find('>') {
                            Some(g) => after + g + 1,
                            None => html.len(),
                        };
                    }
                    None => break,
                }
                continue;
            }

            self.open_tag(&name, tag_src, self_closing);
            i = tag_close + 1;
        }
    }

    fn open_tag(&mut self, name: &str, tag_src: &str, self_closing: bool) {
        // Ẩn theo thuộc tính
        let style = attr_value(tag_src, "style").unwrap_or_default().to_lowercase();
        let hidden_attr = attr_value(tag_src, "hidden").is_some()
            || attr_value(tag_src, "aria-hidden").as_deref() == Some("true");
        let hidden_style = style.split(';').any(|d| {
            let d = d.trim();
            (d.starts_with("display") && d.contains("none"))
                || (d.starts_with("visibility") && (d.contains("hidden") || d.contains("collapse")))
        });

        if !self.skip_stack.is_empty() {
            // Đang bỏ nội dung: chỉ theo dõi thẻ mở của chính nó để cân khớp đóng
            if !self_closing && self.is_container(name) {
                self.skip_stack.push(name.to_string());
            }
            return;
        }

        if DROP_TAGS.contains(&name) || hidden_attr || hidden_style {
            if !self_closing && self.is_container(name) {
                self.skip_stack.push(name.to_string());
            }
            return;
        }

        match name {
            "br" => self.raw_newline(true),
            "hr" => {
                self.ensure_blank_line();
                self.emit("---");
                self.raw_newline(false);
            }
            "p" | "section" | "article" | "main" | "figure" | "figcaption" | "dl" | "dt" | "dd" => {
                self.ensure_blank_line();
            }
            "div" | "span" | "header" | "center" | "small" | "font" => {
                if name == "div" || name == "header" || name == "center" {
                    self.soft_break();
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.heading = name[1..].parse().unwrap_or(2);
                self.ensure_blank_line();
                self.emit(&"#".repeat(self.heading as usize));
                self.emit(" ");
            }
            "strong" | "b" => self.emit("**"),
            "em" | "i" | "cite" | "var" => self.emit("*"),
            "del" | "s" | "strike" => self.emit("~~"),
            "code" | "kbd" | "samp" | "tt" => {
                if self.in_pre {
                    let class = attr_value(tag_src, "class").unwrap_or_default();
                    if let Some(lang) = extract_lang_class(&class) {
                        self.pre_lang = Some(lang);
                    }
                } else {
                    self.emit("`");
                }
            }
            "pre" => {
                self.ensure_blank_line();
                self.in_pre = true;
                self.pre_buf.clear();
                self.pre_lang = None;
            }
            "a" => {
                let href = attr_value(tag_src, "href").unwrap_or_default();
                if href.starts_with("http://") || href.starts_with("https://") || href.starts_with('/') {
                    self.link_href = Some(href);
                    self.emit("[");
                }
            }
            "img" => {
                let src = attr_value(tag_src, "src").unwrap_or_default();
                let alt = attr_value(tag_src, "alt").unwrap_or_default();
                if src.starts_with("http://") || src.starts_with("https://") {
                    let alt = crate::util::truncate_chars(alt.trim(), 120);
                    self.emit(&format!("![{alt}]({src})"));
                    self.soft_break();
                }
            }
            "ul" => {
                self.ensure_blank_line();
                self.list_stack.push(ListKind::Ul);
            }
            "ol" => {
                self.ensure_blank_line();
                let start = attr_value(tag_src, "start")
                    .and_then(|s| s.trim().parse::<usize>().ok())
                    .unwrap_or(1);
                self.list_stack.push(ListKind::Ol(start));
            }
            "li" => {
                self.close_item();
                self.raw_newline(false);
                self.emit(&self.indent());
                let label = match self.list_stack.last_mut() {
                    Some(ListKind::Ol(n)) => {
                        *n += 1;
                        format!("{}. ", *n - 1)
                    }
                    _ => "- ".to_string(),
                };
                self.emit(&label);
                self.list_item_open = true;
            }
            "blockquote" => {
                self.ensure_blank_line();
                self.quote_depth += 1;
                self.emit(&"> ".repeat(self.quote_depth));
            }
            "table" => {
                self.ensure_blank_line();
                self.table_rows.clear();
                self.table_done_header = false;
                self.pending_row.clear();
            }
            "tr" => {
                self.flush_row();
            }
            "td" | "th" => {
                if self.cell.is_none() {
                    self.cell = Some(String::new());
                    self.cell_is_header = name == "th";
                }
            }
            _ => {}
        }
    }

    fn close_tag(&mut self, name: &str) {
        if !self.skip_stack.is_empty() {
            if self.skip_stack.last().map(|s| s.as_str()) == Some(name) {
                self.skip_stack.pop();
            }
            return;
        }
        match name {
            "p" | "section" | "article" | "main" | "figure" | "figcaption" | "dl" | "dt" | "dd" => {
                self.ensure_blank_line();
            }
            "div" | "header" | "center" => self.soft_break(),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.heading = 0;
                self.raw_newline(false);
                self.ensure_blank_line();
            }
            "strong" | "b" => self.emit("**"),
            "em" | "i" | "cite" | "var" => self.emit("*"),
            "del" | "s" | "strike" => self.emit("~~"),
            "code" | "kbd" | "samp" | "tt" => {
                if !self.in_pre {
                    self.emit("`");
                }
            }
            "pre" => {
                self.in_pre = false;
                let lang = self.pre_lang.take().unwrap_or_default();
                let body = std::mem::take(&mut self.pre_buf);
                let body = body.trim_end();
                self.emit(&format!("\n```{lang}\n{body}\n```"));
                self.raw_newline(false);
                self.ensure_blank_line();
            }
            "a" => {
                if let Some(href) = self.link_href.take() {
                    self.emit(&format!("]({href})"));
                }
            }
            "ul" | "ol" => {
                self.close_item();
                self.list_stack.pop();
                self.ensure_blank_line();
            }
            "li" => {
                self.close_item();
            }
            "blockquote" => {
                self.quote_depth = self.quote_depth.saturating_sub(1);
                self.ensure_blank_line();
            }
            "td" | "th" => {
                if let Some(cell) = self.cell.take() {
                    let cleaned = cell.trim().replace('|', "\\|");
                    self.pending_row.push(cleaned);
                }
            }
            "tr" => {
                self.flush_row();
            }
            "table" => {
                self.flush_row();
                let rows = std::mem::take(&mut self.table_rows);
                if !rows.is_empty() {
                    for (ri, row) in rows.iter().enumerate() {
                        if row.is_empty() {
                            continue;
                        }
                        let line = row.iter().map(|c| format!(" {c} ")).collect::<Vec<_>>().join("|");
                        self.emit(&format!("|{line}|"));
                        self.raw_newline(false);
                        if ri == 0 {
                            let sep = row.iter().map(|_| " --- ").collect::<Vec<_>>().join("|");
                            self.emit(&format!("|{sep}|"));
                            self.raw_newline(false);
                        }
                    }
                }
                self.table_done_header = false;
                self.ensure_blank_line();
            }
            _ => {}
        }
    }

    fn close_item(&mut self) {
        if self.list_item_open {
            self.list_item_open = false;
            self.raw_newline(false);
        }
    }

    fn indent(&self) -> String {
        let depth = self.list_stack.len().saturating_sub(1);
        "  ".repeat(depth)
    }

    fn flush_row(&mut self) {
        if !self.pending_row.is_empty() {
            let row = std::mem::take(&mut self.pending_row);
            self.table_rows.push(row);
        }
    }

    fn text(&mut self, text: &str) {
        if !self.skip_stack.is_empty() {
            return;
        }
        if self.in_pre {
            self.pre_buf.push_str(text);
            return;
        }
        let decoded = decode_entities(text);
        let collapsed = collapse_ws(&decoded);
        if collapsed.trim().is_empty() {
            return;
        }
        self.emit_text(&collapsed);
    }

    /// Ghi nội dung: thụt đầu dòng và marker đi thẳng, văn bản thì bỏ khoảng trắng thừa đầu dòng.
    fn emit(&mut self, s: &str) {
        if let Some(cell) = self.cell.as_mut() {
            cell.push_str(s);
            return;
        }
        self.out.push_str(s);
    }

    fn emit_text(&mut self, s: &str) {
        if let Some(cell) = self.cell.as_mut() {
            let t = s.trim();
            if !t.is_empty() {
                cell.push_str(t);
            }
            return;
        }
        if self.out.is_empty() || self.out.ends_with('\n') {
            let trimmed = s.trim_start();
            if !trimmed.is_empty() {
                self.out.push_str(trimmed);
            }
        } else {
            self.out.push_str(s);
        }
    }

    fn raw_newline(&mut self, _single: bool) {
        if let Some(cell) = self.cell.as_mut() {
            cell.push(' ');
            return;
        }
        if !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        if self.quote_depth > 0 {
            self.out.push_str(&"> ".repeat(self.quote_depth));
        }
    }

    fn soft_break(&mut self) {
        if self.cell.is_some() {
            return;
        }
        if !self.out.ends_with('\n') && !self.out.ends_with(' ') {
            self.out.push('\n');
        }
    }

    fn ensure_blank_line(&mut self) {
        if self.cell.is_some() {
            return;
        }
        while self.out.ends_with('\n') {
            self.out.pop();
        }
        if !self.out.is_empty() {
            self.out.push_str("\n\n");
        }
        if self.quote_depth > 0 {
            self.out.push_str(&"> ".repeat(self.quote_depth));
        }
    }

    /// Chỉ dùng cho thẻ có thể chứa thẻ con (để cân bằng skip_stack).
    fn is_container(&self, name: &str) -> bool {
        !matches!(
            name,
            "br" | "hr" | "img" | "input" | "meta" | "link" | "source" | "col" | "area" | "base" | "param" | "track" | "wbr" | "embed"
        )
    }
}

fn extract_lang_class(class: &str) -> Option<String> {
    for token in class.split_whitespace() {
        if let Some(lang) = token.strip_prefix("language-") {
            if !lang.is_empty() {
                return Some(lang.to_string());
            }
        }
    }
    None
}

// =====================================================================
// Tiện ích chung
// =====================================================================

/// Giải mã entity HTML, giữ nguyên khoảng trắng.
pub fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '&' {
            if let Some(semi) = chars[i + 1..].iter().position(|&c| c == ';') {
                let ent: String = chars[i + 1..i + 1 + semi].iter().collect();
                let rep = match ent.as_str() {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" | "#39" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => {
                        if let Some(hex) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) {
                            u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                        } else if let Some(dec) = ent.strip_prefix('#') {
                            dec.parse::<u32>().ok().and_then(char::from_u32)
                        } else {
                            None
                        }
                    }
                };
                if let Some(c) = rep {
                    out.push(c);
                    i += semi + 2;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Co dãy khoảng trắng thành một dấu cách.
pub fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !in_ws {
                out.push(' ');
                in_ws = true;
            }
        } else {
            out.push(c);
            in_ws = false;
        }
    }
    out
}

// =====================================================================
// Test
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("https%3A%2F%2Fexample.com%2Fa%3Fb%3D1"), "https://example.com/a?b=1");
        assert_eq!(percent_decode("a+b"), "a b");
    }

    #[test]
    fn test_resolve_ddg_href() {
        let h = "//duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org%2F&rut=abc";
        assert_eq!(resolve_ddg_href(h).unwrap(), "https://rust-lang.org/");
        assert_eq!(resolve_ddg_href("https://example.com").unwrap(), "https://example.com");
        assert!(resolve_ddg_href("/settings").is_none());
    }

    #[test]
    fn test_parse_ddg() {
        let html = r##"
        <div class="result">
          <h2 class="result__title">
            <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fa">Example <b>Domain</b></a>
          </h2>
          <a class="result__snippet" href="#">This domain is for use in &amp; examples</a>
        </div>
        <div class="result">
          <h2 class="result__title">
            <a rel="nofollow" class="result__a" href="https://other.org">Other site</a>
          </h2>
        </div>
        "##;
        let hits = parse_ddg_results(html);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].title, "Example Domain");
        assert_eq!(hits[0].url, "https://example.com/a");
        assert_eq!(hits[0].snippet, "This domain is for use in & examples");
        assert_eq!(hits[1].url, "https://other.org");
    }

    #[test]
    fn test_parse_ddg_lite() {
        let html = r#"
        <a rel="nofollow" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Ffoo.dev%2Fdoc" class="result-link">Foo Docs</a>
        <td class="result-snippet">The best docs</td>
        "#;
        let hits = parse_ddg_results(html);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].url, "https://foo.dev/doc");
    }

    #[test]
    fn test_html_to_md_basic() {
        let md = html_to_markdown("<h1>Tiêu đề</h1><p>Đoạn <strong>đậm</strong> và <em>nghiêng</em>.</p>");
        assert!(md.contains("# Tiêu đề"), "{md}");
        assert!(md.contains("**đậm**"), "{md}");
        assert!(md.contains("*nghiêng*"), "{md}");
    }

    #[test]
    fn test_html_to_md_link() {
        let md = html_to_markdown(r#"<p>Xem <a href="https://example.com" title="x">trang này</a> nhé</p>"#);
        assert!(md.contains("[trang này](https://example.com)"), "{md}");
    }

    #[test]
    fn test_html_to_md_list() {
        let md = html_to_markdown("<ul><li>một</li><li>hai<ul><li>con</li></ul></li></ul>");
        assert!(md.contains("- một"), "{md}");
        assert!(md.contains("- hai"), "{md}");
        assert!(md.contains("  - con"), "{md}");
    }

    #[test]
    fn test_html_to_md_ol() {
        let md = html_to_markdown("<ol start=\"3\"><li>ba</li><li>bốn</li></ol>");
        assert!(md.contains("3. ba"), "{md}");
        assert!(md.contains("4. bốn"), "{md}");
    }

    #[test]
    fn test_html_to_md_code() {
        let md = html_to_markdown("<p>code <code>x = 1</code> đây</p><pre><code class=\"language-rust\">fn main() {}</code></pre>");
        assert!(md.contains("`x = 1`"), "{md}");
        assert!(md.contains("```rust\nfn main() {}\n```"), "{md}");
    }

    #[test]
    fn test_html_to_md_table() {
        let md = html_to_markdown("<table><tr><th>Tên</th><th>Tuổi</th></tr><tr><td>An</td><td>5</td></tr></table>");
        assert!(md.contains("| Tên | Tuổi |"), "{md}");
        assert!(md.contains("| --- | --- |"), "{md}");
        assert!(md.contains("| An | 5 |"), "{md}");
    }

    #[test]
    fn test_html_to_md_hidden() {
        let md = html_to_markdown("<p>hi</p><div style=\"display:none\">secret</div><p>there</p>");
        assert!(!md.contains("secret"), "{md}");
        assert!(md.contains("hi"), "{md}");
        assert!(md.contains("there"), "{md}");
    }

    #[test]
    fn test_html_to_md_drops() {
        let md = html_to_markdown("<script>var x = 1 < 2;</script><style>.a{}</style><nav>menu</nav><p>ok</p>");
        assert_eq!(md, "ok");
    }

    #[test]
    fn test_html_to_md_blockquote() {
        let md = html_to_markdown("<blockquote>trích <b>dẫn</b></blockquote>");
        assert!(md.contains("> trích **dẫn**"), "{md}");
    }

    #[test]
    fn test_html_to_md_entities() {
        let md = html_to_markdown("<p>a &amp; b &lt; c &#x1F600;</p>");
        assert!(md.contains("a & b < c"), "{md}");
    }

    #[test]
    fn test_parse_bing_rss() {
        let xml = r#"<?xml version="1.0" encoding="utf-8" ?>
<rss version="2.0"><channel><title>Bing: rust</title>
<item><title>Rust Programming Language</title><link>https://www.rust-lang.org/</link><description>A language empowering everyone &amp; to build reliable software.</description></item>
<item><title>Rust on Wikipedia</title><link>https://en.wikipedia.org/wiki/Rust_(programming_language)</link><description>Rust is a multi-paradigm language.</description></item>
<item><title>Bing internal</title><link>https://www.bing.com/aclick</link><description>ad</description></item>
</channel></rss>"#;
        let hits = parse_bing_rss(xml);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].title, "Rust Programming Language");
        assert_eq!(hits[0].url, "https://www.rust-lang.org/");
        assert!(hits[0].snippet.contains("everyone &"));
        assert_eq!(hits[1].url, "https://en.wikipedia.org/wiki/Rust_(programming_language)");
    }

    #[test]
    fn test_private_hosts() {
        assert!(host_is_private("localhost"));
        assert!(host_is_private("192.168.1.1"));
        assert!(host_is_private("10.0.0.5"));
        assert!(host_is_private("169.254.169.254"));
        assert!(!host_is_private("example.com"));
        assert!(!host_is_private("8.8.8.8"));
    }

    #[test]
    fn test_offsets() {
        assert_eq!(resolve_timezone("+7").unwrap().whole_hours(), 7);
        assert_eq!(resolve_timezone("-05:30").unwrap().minutes_past_hour().abs(), 30);
        assert_eq!(resolve_timezone("GMT+9").unwrap().whole_hours(), 9);
        assert!(resolve_timezone("Asia/Ho_Chi_Minh").is_ok());
        assert!(resolve_timezone("America/New_York").is_ok());
        assert!(resolve_timezone("Not/AZone").is_err());
    }

    #[test]
    fn test_datetime_output() {
        let args = json!({});
        let r = datetime(&args, "vi");
        assert!(r.ok);
        assert!(r.output.contains("ISO 8601"), "{}", r.output);
        assert!(r.output.contains("Unix timestamp"));
        assert!(r.summary.contains("GMT+7"));
    }

    #[test]
    fn test_format_thousands() {
        assert_eq!(format_thousands(1234), "1,234");
        assert_eq!(format_thousands(123456), "123,456");
        assert_eq!(format_thousands(42), "42");
    }
}
