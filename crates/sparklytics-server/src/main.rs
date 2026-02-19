use std::sync::Arc;

use anyhow::Result;
use tracing::info;

use sparklytics_server::state::AppState;

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

    // Seed a default website so the server is usable out of the box.
    // Uses ON CONFLICT so it's safe to run on every startup.
    if let Err(e) = db.seed_website("site_default", "localhost").await {
        tracing::warn!(error = %e, "Failed to seed default website");
    } else {
        info!("Default website 'site_default' (localhost) ready");
    }

    // Auth initialization for password/local modes.
    match &cfg.auth_mode {
        sparklytics_core::config::AuthMode::Password(_) | sparklytics_core::config::AuthMode::Local => {
            match db.ensure_jwt_secret().await {
                Ok(_) => info!("JWT secret ready"),
                Err(e) => tracing::error!(error = %e, "Failed to ensure JWT secret"),
            }

            if let sparklytics_core::config::AuthMode::Local = &cfg.auth_mode {
                match db.is_admin_configured().await {
                    Ok(true) => info!("Admin password configured"),
                    Ok(false) => info!("Admin not configured — setup required via POST /api/auth/setup"),
                    Err(e) => tracing::error!(error = %e, "Failed to check admin configured"),
                }
            }

            info!(auth_mode = ?cfg.auth_mode, "Auth enabled");
        }
        sparklytics_core::config::AuthMode::None => {
            info!("Auth disabled (SPARKLYTICS_AUTH=none) — all routes open");
        }
    }

    let state = Arc::new(AppState::new(db, cfg.clone()));

    // Spawn background buffer-flush task.
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            state.run_buffer_flush_loop().await;
        });
    }

    // Spawn background daily salt rotation task (rotates at midnight UTC).
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            state.run_salt_rotation_loop().await;
        });
    }

    let addr = format!("0.0.0.0:{}", cfg.port);
    let app = sparklytics_server::app::build_app(Arc::clone(&state));

    info!(port = cfg.port, mode = ?cfg.mode, "Sparklytics listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
