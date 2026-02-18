use std::sync::Arc;

use anyhow::Result;
use tracing::info;

mod app;
mod config;
mod error;
mod routes;
mod state;

use state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialise structured JSON logging. Level controlled via RUST_LOG env var.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sparklytics=info".parse()?),
        )
        .json()
        .init();

    let cfg = sparklytics_core::config::Config::from_env().map_err(|e| anyhow::anyhow!(e))?;

    // Ensure data directory exists before opening DuckDB.
    std::fs::create_dir_all(&cfg.data_dir)?;
    let db_path = format!("{}/sparklytics.db", cfg.data_dir);

    // Open DuckDB — initialises schema and seeds settings table.
    let db = sparklytics_duckdb::DuckDbBackend::open(&db_path)?;

    // Log a warning (not panic) if GeoIP database is absent — Sprint 0 requirement.
    // Events will be stored with NULL country/region/city fields.
    if !std::path::Path::new(&cfg.geoip_path).exists() {
        tracing::warn!(
            geoip_path = %cfg.geoip_path,
            "GeoIP database not found. Events stored with NULL geo fields. \
             Download GeoLite2-City.mmdb from MaxMind and set SPARKLYTICS_GEOIP_PATH."
        );
    }

    let state = Arc::new(AppState::new(db, cfg.clone()));

    // Spawn background buffer-flush task.
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            state.run_buffer_flush_loop().await;
        });
    }

    let addr = format!("0.0.0.0:{}", cfg.port);
    let app = app::build_app(Arc::clone(&state));

    info!(port = cfg.port, mode = ?cfg.mode, "Sparklytics listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
