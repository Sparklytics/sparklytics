use std::sync::Arc;

use anyhow::Result;
use tracing::info;

use sparklytics_server::state::AppState;

/// `sparklytics health` — liveness probe for Docker HEALTHCHECK.
///
/// Calls `GET http://localhost:$SPARKLYTICS_PORT/health`.
/// Exits 0 if the server responds with HTTP 200, exits 1 otherwise.
fn run_health_check() -> ! {
    let port = std::env::var("SPARKLYTICS_PORT").unwrap_or_else(|_| "3000".to_string());
    let url = format!("http://localhost:{}/health", port);
    match ureq::get(&url).call() {
        Ok(resp) if resp.status() == 200 => std::process::exit(0),
        _ => std::process::exit(1),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Health-check subcommand — must be handled before tokio runtime initialisation
    // so the binary stays small and fast when used as a Docker HEALTHCHECK probe.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("health") {
        run_health_check();
    }
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
    let db = sparklytics_duckdb::DuckDbBackend::open(&db_path, &cfg.duckdb_memory_limit)?;

    // Log a warning (not panic) if GeoIP database is absent — Sprint 0 requirement.
    // Events will be stored with NULL country/region/city fields.
    if !std::path::Path::new(&cfg.geoip_path).exists() {
        tracing::warn!(
            geoip_path = %cfg.geoip_path,
            "GeoIP database not found. Events stored with NULL geo fields. \
             Run scripts/download-geoip.sh to fetch DB-IP City Lite (free, no key required), \
             then set SPARKLYTICS_GEOIP_PATH. Docker images bundle DB-IP automatically."
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
        sparklytics_core::config::AuthMode::Password(_)
        | sparklytics_core::config::AuthMode::Local => {
            match db.ensure_jwt_secret().await {
                Ok(_) => info!("JWT secret ready"),
                Err(e) => tracing::error!(error = %e, "Failed to ensure JWT secret"),
            }

            if let sparklytics_core::config::AuthMode::Local = &cfg.auth_mode {
                match db.is_admin_configured().await {
                    Ok(true) => info!("Admin password configured"),
                    Ok(false) => {
                        info!("Admin not configured — setup required via POST /api/auth/setup")
                    }
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
    let state_for_shutdown = Arc::clone(&state);
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;

    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        state_for_shutdown.flush_buffer(),
    )
    .await
    .ok();

    Ok(())
}
