use serde_json::{json, Value as Json};

/// Dinh nghia cac cong cu trinh bay cho model.
pub fn schemas() -> Json {
    json!([
        {
            "type": "function",
            "function": {
                "name": "current_time",
                "description": "Lấy thời gian hiện tại theo múi giờ của người dùng (GMT+7). Dùng khi cần biết ngày, giờ hiện tại.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "calculate",
                "description": "Tính toán một biểu thức toán học. Hỗ trợ + - * / % ^, ngoặc, và các hàm sqrt, sin, cos, tan, log, ln, abs, round, floor, ceil, exp, min, max, hằng số pi, e.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "expression": { "type": "string", "description": "Biểu thức cần tính, ví dụ: (12 + 8) * 3.5" }
                    },
                    "required": ["expression"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "fetch_url",
                "description": "Đọc nội dung văn bản của một trang web công khai và trả về текст đã làm sạch. Chỉ dùng khi người dùng yêu cầu tra cứu thông tin từ một đường dẫn cụ thể.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "Đường dẫn http(s) cần đọc" }
                    },
                    "required": ["url"]
                }
            }
        }
    ])
}

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

pub async fn execute(name: &str, args: &Json) -> ToolResult {
    match name {
        "current_time" => {
            let now = crate::util::format_full(crate::util::local_now());
            ToolResult::ok(now.clone(), now)
        }
        "calculate" => {
            let expr = args.get("expression").and_then(|e| e.as_str()).unwrap_or("").trim();
            if expr.is_empty() {
                return ToolResult::err("Thiếu biểu thức".into());
            }
            if expr.len() > 500 {
                return ToolResult::err("Biểu thức quá dài".into());
            }
            match eval_math(expr) {
                Ok(v) => {
                    let out = format_number(v);
                    ToolResult::ok(format!("= {out}"), format!("{expr} = {out}"))
                }
                Err(e) => ToolResult::err(format!("Không tính được: {e}")),
            }
        }
        "fetch_url" => {
            let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("").trim();
            fetch_url(url).await
        }
        _ => ToolResult::err(format!("Công cụ không tồn tại: {name}")),
    }
}

fn format_number(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

// ============== Bo tinh toan an toan (khong dung eval) ==============

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    LParen,
    RParen,
    Comma,
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '0'..='9' | '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                // so mu khoa hoc 1e5
                if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                    let mut j = i + 1;
                    if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                        j += 1;
                    }
                    if j < chars.len() && chars[j].is_ascii_digit() {
                        i = j;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                }
                let text: String = chars[start..i].iter().collect();
                let n: f64 = text.parse().map_err(|_| format!("số không hợp lệ: {text}"))?;
                out.push(Tok::Num(n));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                out.push(Tok::Ident(chars[start..i].iter().collect()));
            }
            '+' | '-' | '*' | '/' | '%' | '^' => {
                out.push(Tok::Op(c));
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            _ => return Err(format!("ký tự không hợp lệ: {c}")),
        }
    }
    Ok(out)
}

struct MathParser {
    toks: Vec<Tok>,
    pos: usize,
}

impl MathParser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn parse_expr(&mut self) -> Result<f64, String> {
        let mut left = self.parse_term()?;
        loop {
            match self.peek() {
                Some(Tok::Op('+')) => {
                    self.pos += 1;
                    left += self.parse_term()?;
                }
                Some(Tok::Op('-')) => {
                    self.pos += 1;
                    left -= self.parse_term()?;
                }
                _ => return Ok(left),
            }
        }
    }

    fn parse_term(&mut self) -> Result<f64, String> {
        let mut left = self.parse_unary()?;
        loop {
            match self.peek() {
                Some(Tok::Op('*')) => {
                    self.pos += 1;
                    left *= self.parse_unary()?;
                }
                Some(Tok::Op('/')) => {
                    self.pos += 1;
                    let r = self.parse_unary()?;
                    if r == 0.0 {
                        return Err("chia cho 0".into());
                    }
                    left /= r;
                }
                Some(Tok::Op('%')) => {
                    self.pos += 1;
                    let r = self.parse_unary()?;
                    if r == 0.0 {
                        return Err("chia cho 0".into());
                    }
                    left %= r;
                }
                _ => return Ok(left),
            }
        }
    }

    fn parse_unary(&mut self) -> Result<f64, String> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.pos += 1;
                Ok(-self.parse_unary()?)
            }
            Some(Tok::Op('+')) => {
                self.pos += 1;
                self.parse_unary()
            }
            _ => self.parse_power(),
        }
    }

    fn parse_power(&mut self) -> Result<f64, String> {
        let base = self.parse_atom()?;
        if matches!(self.peek(), Some(Tok::Op('^'))) {
            self.pos += 1;
            let exp = self.parse_unary()?; // ket hop phai
            Ok(base.powf(exp))
        } else {
            Ok(base)
        }
    }

    fn parse_atom(&mut self) -> Result<f64, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(n),
            Some(Tok::LParen) => {
                let v = self.parse_expr()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(v),
                    other => Err(format!("thiếu ')' , gặp {other:?}")),
                }
            }
            Some(Tok::Ident(name)) => {
                let lname = name.to_lowercase();
                let constant = match lname.as_str() {
                    "pi" => Some(std::f64::consts::PI),
                    "e" => Some(std::f64::consts::E),
                    _ => None,
                };
                if let Some(c) = constant {
                    if !matches!(self.peek(), Some(Tok::LParen)) {
                        return Ok(c);
                    }
                }
                match self.next() {
                    Some(Tok::LParen) => {}
                    other => return Err(format!("thiếu '(' sau tên hàm, gặp {other:?}")),
                }
                let mut args = vec![self.parse_expr()?];
                while matches!(self.peek(), Some(Tok::Comma)) {
                    self.pos += 1;
                    args.push(self.parse_expr()?);
                }
                match self.next() {
                    Some(Tok::RParen) => {}
                    other => return Err(format!("thiếu ')' , gặp {other:?}")),
                }
                apply_fn(&lname, &args)
            }
            other => Err(format!("token không hợp lệ: {other:?}")),
        }
    }
}

fn apply_fn(name: &str, args: &[f64]) -> Result<f64, String> {
    let arg = |i: usize| -> Result<f64, String> {
        args.get(i).copied().ok_or_else(|| format!("{name} cần {} đối số", i + 1))
    };
    let one = |f: fn(f64) -> f64| -> Result<f64, String> {
        Ok(f(arg(0)?))
    };
    match name {
        "sqrt" => {
            let v = arg(0)?;
            if v < 0.0 {
                return Err("căn bậc hai của số âm".into());
            }
            Ok(v.sqrt())
        }
        "sin" => one(f64::sin),
        "cos" => one(f64::cos),
        "tan" => one(f64::tan),
        "asin" => one(f64::asin),
        "acos" => one(f64::acos),
        "atan" => one(f64::atan),
        "ln" => {
            let v = arg(0)?;
            if v <= 0.0 {
                return Err("logarithm của số không dương".into());
            }
            Ok(v.ln())
        }
        "log" => {
            let v = arg(0)?;
            if v <= 0.0 {
                return Err("logarithm của số không dương".into());
            }
            Ok(v.log10())
        }
        "abs" => one(f64::abs),
        "round" => one(f64::round),
        "floor" => one(f64::floor),
        "ceil" => one(f64::ceil),
        "exp" => one(f64::exp),
        "min" => Ok(arg(0)?.min(arg(1)?)),
        "max" => Ok(arg(0)?.max(arg(1)?)),
        _ => Err(format!("không biết hàm {name}")),
    }
}

pub fn eval_math(expr: &str) -> Result<f64, String> {
    let toks = tokenize(expr)?;
    if toks.is_empty() {
        return Err("biểu thức rỗng".into());
    }
    let mut p = MathParser { toks, pos: 0 };
    let v = p.parse_expr()?;
    if p.pos != p.toks.len() {
        return Err("còn thừa token sau biểu thức".into());
    }
    if !v.is_finite() {
        return Err("kết quả không xác định".into());
    }
    Ok(v)
}

// ============== fetch_url ==============

fn host_is_private(host: &str) -> bool {
    let h = host.trim_end_matches('.').to_lowercase();
    if h == "localhost" || h.ends_with(".local") || h.ends_with(".internal") || h.ends_with(".lan") {
        return true;
    }
    // IPv4 literal
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
    // IPv6 literal don gian
    if h.contains(':') && (h.starts_with("fc") || h.starts_with("fd") || h.starts_with("fe80") || h == "::1") {
        return true;
    }
    false
}

fn strip_html(html: &str) -> String {
    let lower = html.to_lowercase();
    let mut out = String::with_capacity(html.len() / 2);
    let mut i = 0usize;
    let n = html.len();
    while i < n {
        let rest = &lower[i..];
        // Bo toan bo khoi script/style (kem noi dung)
        if rest.starts_with("<script") {
            match rest.find("</script") {
                Some(end) => {
                    i += end + "</script".len();
                    if lower[i..].starts_with('>') {
                        i += 1;
                    }
                    continue;
                }
                None => break,
            }
        }
        if rest.starts_with("<style") {
            match rest.find("</style") {
                Some(end) => {
                    i += end + "</style".len();
                    if lower[i..].starts_with('>') {
                        i += 1;
                    }
                    continue;
                }
                None => break,
            }
        }
        match rest.find('<') {
            Some(rel) => {
                match html.get(i..i + rel) {
                    Some(text) => out.push_str(text),
                    None => break,
                }
                i += rel;
                match lower[i..].find('>') {
                    Some(rel_gt) => i += rel_gt + 1,
                    None => break,
                }
            }
            None => {
                match html.get(i..) {
                    Some(text) => out.push_str(text),
                    None => break,
                }
                break;
            }
        }
    }
    decode_entities(&out)
}

fn decode_entities(s: &str) -> String {
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
    // Gon khoang trang
    let collapsed: String = out.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
}

async fn fetch_url(url: &str) -> ToolResult {
    if url.len() > 2000 {
        return ToolResult::err("Đường dẫn quá dài".into());
    }
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return ToolResult::err("Đường dẫn không hợp lệ".into());
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return ToolResult::err("Chỉ hỗ trợ http/https".into());
    }
    let Some(host) = parsed.host_str() else {
        return ToolResult::err("Thiếu tên miền".into());
    };
    if host_is_private(host) {
        return ToolResult::err("Không được truy cập địa chỉ nội bộ".into());
    }

    let http = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(4))
        .user_agent("Mozilla/5.0 (compatible; HalisBot/0.1)")
        .build()
        .unwrap();
    let resp = match http.get(parsed.clone()).send().await {
        Ok(r) => r,
        Err(_) => return ToolResult::err("Không truy cập được trang web".into()),
    };
    let status = resp.status();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    let Ok(bytes) = resp.bytes().await else {
        return ToolResult::err("Không đọc được nội dung".into());
    };
    let ct_ok = content_type.contains("text/")
        || content_type.contains("json")
        || content_type.contains("xml")
        || content_type.contains("javascript");
    if !ct_ok {
        return ToolResult::err(format!("Không hỗ trợ loại nội dung: {content_type}"));
    }
    let limited = &bytes[..bytes.len().min(512 * 1024)];
    let text = String::from_utf8_lossy(limited);
    let clean = if content_type.contains("html") { strip_html(&text) } else { text.to_string() };
    let clean = clean.trim().to_string();
    if clean.is_empty() {
        return ToolResult::err("Trang không có nội dung văn bản".into());
    }
    let (preview, truncated) = if clean.chars().count() > 8000 {
        (crate::util::truncate_chars(&clean, 8000), true)
    } else {
        (clean.clone(), false)
    };
    let summary = format!("{host} · {} ký tự{}", clean.chars().count(), if truncated { " (cắt ngắn)" } else { "" });
    ToolResult::ok(
        format!("URL: {url}\nHTTP {status}\n\n{preview}"),
        summary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_basic() {
        assert_eq!(eval_math("1+2*3").unwrap(), 7.0);
        assert_eq!(eval_math("(1+2)*3").unwrap(), 9.0);
        assert_eq!(eval_math("2^10").unwrap(), 1024.0);
        assert_eq!(eval_math("-5+3").unwrap(), -2.0);
        assert_eq!(eval_math("10/4").unwrap(), 2.5);
        assert_eq!(eval_math("7%3").unwrap(), 1.0);
    }

    #[test]
    fn test_eval_funcs() {
        assert_eq!(eval_math("sqrt(16)").unwrap(), 4.0);
        assert_eq!(eval_math("max(3, 9)").unwrap(), 9.0);
        assert!((eval_math("sin(0)").unwrap()).abs() < 1e-12);
        assert!((eval_math("pi*2").unwrap() - std::f64::consts::TAU).abs() < 1e-12);
    }

    #[test]
    fn test_eval_errors() {
        assert!(eval_math("1/0").is_err());
        assert!(eval_math("sqrt(-1)").is_err());
        assert!(eval_math("foo(1)").is_err());
        assert!(eval_math("").is_err());
        assert!(eval_math("<script>").is_err());
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
    fn test_strip_html() {
        assert_eq!(strip_html("<p>Xin &amp; ch&#224;o</p>"), "Xin & chào");
        assert_eq!(strip_html("<script>bad()</script>ok"), "ok");
    }
}
