use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub base_url: String,
    pub bind_addr: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub ai_base_url: String,
    pub ai_model: String,
    pub ai_max_output_tokens: u32,
    pub admin_emails: Vec<String>,
    pub dev_login: bool,
    pub secure_cookies: bool,
}

fn var(key: &str, default: &str) -> String {
    env::var(key).ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| default.to_string())
}

impl Config {
    pub fn from_env() -> Self {
        let base_url = var("APP_BASE_URL", "http://localhost:3000")
            .trim_end_matches('/')
            .to_string();
        let secure_cookies = base_url.starts_with("https://");
        let dev_login = var("DEV_LOGIN", "false") == "true";
        if dev_login {
            tracing::warn!("DEV_LOGIN dang bat - chi dung khi phat trien, tat o production!");
        }
        let admin_emails: Vec<String> = var("ADMIN_EMAILS", "")
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        Self {
            database_url: var("DATABASE_URL", "postgres://postgres:postgres@localhost:5432/halisai"),
            secure_cookies,
            base_url,
            bind_addr: var("BIND_ADDR", "0.0.0.0:3000"),
            google_client_id: var("GOOGLE_CLIENT_ID", ""),
            google_client_secret: var("GOOGLE_CLIENT_SECRET", ""),
            ai_base_url: var("AI_BASE_URL", "https://api.kilo.ai/api/gateway"),
            ai_model: var("AI_MODEL", "kilo-auto/free"),
            ai_max_output_tokens: var("AI_MAX_OUTPUT_TOKENS", "32768").parse().unwrap_or(32768),
            admin_emails,
            dev_login,
        }
    }
}
