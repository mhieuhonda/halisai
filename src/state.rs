use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sqlx::PgPool;

use crate::ai::AiClient;
use crate::config::Config;
use crate::tools::Tools;

/// Gioi han don gian theo cua so truot.
pub struct RateLimiter {
    map: Mutex<HashMap<uuid::Uuid, VecDeque<Instant>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self { map: Mutex::new(HashMap::new()) }
    }

    pub fn allow(&self, key: uuid::Uuid, max: usize, window: Duration) -> bool {
        let mut map = self.map.lock().unwrap();
        let now = Instant::now();
        let q = map.entry(key).or_default();
        while let Some(front) = q.front() {
            if now.duration_since(*front) > window {
                q.pop_front();
            } else {
                break;
            }
        }
        if q.len() >= max {
            return false;
        }
        q.push_back(now);
        true
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cfg: Arc<Config>,
    pub http: reqwest::Client,
    pub ai: Arc<AiClient>,
    pub tools: Arc<Tools>,
    pub rl: Arc<RateLimiter>,
}
