use anyhow::Result;
use sqlx::PgPool;

/// Create a PostgreSQL connection pool from the `DATABASE_URL` env var.
///
/// Called once at startup in cloud mode. Runs `sqlx::migrate!()` immediately
/// after connecting to apply any pending migrations.
pub async fn create_pool(database_url: &str) -> Result<PgPool> {
    let pool = PgPool::connect(database_url).await?;
    sqlx::migrate!("../../migrations").run(&pool).await?;
    Ok(pool)
}
