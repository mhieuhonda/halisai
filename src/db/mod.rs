pub mod chat;
pub mod misc;
pub mod session;
pub mod user;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn init_pool(database_url: &str) -> PgPool {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(database_url)
        .await
        .unwrap_or_else(|e| panic!("khong ket noi duoc postgres: {e}"));
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("chay migration that bai");
    pool
}
