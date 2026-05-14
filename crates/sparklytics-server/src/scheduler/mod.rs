use std::sync::Arc;
use std::time::Duration;

use tracing::{error, info};

use crate::state::AppState;

pub mod alerts;
pub mod delivery;
pub mod subscriptions;

fn scheduler_tick_seconds() -> u64 {
    std::env::var("SPARKLYTICS_SCHEDULER_TICK_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(|v| v.clamp(10, 3600))
        .unwrap_or(60)
}

pub async fn process_once(state: &Arc<AppState>) -> anyhow::Result<(usize, usize)> {
    let subscription_runs = subscriptions::run_due_subscriptions(state).await?;
    let alert_deliveries = alerts::run_alert_checks(state).await?;
    Ok((subscription_runs, alert_deliveries))
}

pub async fn prune_retention_once(
    state: &Arc<AppState>,
) -> anyhow::Result<sparklytics_duckdb::RetentionPruneStats> {
    state
        .db
        .prune_analytics_retention(state.config.retention_days)
        .await
}

pub async fn run_startup_maintenance(
    state: &Arc<AppState>,
) -> (
    anyhow::Result<u64>,
    anyhow::Result<sparklytics_duckdb::RetentionPruneStats>,
) {
    let login_attempts = state.metadata.prune_login_attempts().await;
    let retention = prune_retention_once(state).await;
    (login_attempts, retention)
}

pub async fn run_scheduler_loop(state: Arc<AppState>) {
    let tick = scheduler_tick_seconds();
    info!(tick_seconds = tick, "Notifications scheduler started");
    let mut interval = tokio::time::interval(Duration::from_secs(tick));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_login_attempt_prune = std::time::Instant::now();
    let mut last_retention_prune = std::time::Instant::now();
    loop {
        interval.tick().await;
        if last_login_attempt_prune.elapsed() >= Duration::from_secs(24 * 60 * 60) {
            match state.metadata.prune_login_attempts().await {
                Ok(pruned) if pruned > 0 => info!(pruned, "Pruned stale login attempts"),
                Ok(_) => {}
                Err(err) => error!(error = %err, "Failed to prune stale login attempts"),
            }
            last_login_attempt_prune = std::time::Instant::now();
        }
        if last_retention_prune.elapsed() >= Duration::from_secs(24 * 60 * 60) {
            match prune_retention_once(&state).await {
                Ok(stats) if stats.events_deleted > 0 || stats.sessions_deleted > 0 => info!(
                    events_deleted = stats.events_deleted,
                    sessions_deleted = stats.sessions_deleted,
                    retention_days = state.config.retention_days,
                    "Pruned analytics rows beyond retention horizon"
                ),
                Ok(_) => {}
                Err(err) => error!(error = %err, "Failed to prune analytics retention"),
            }
            last_retention_prune = std::time::Instant::now();
        }
        if let Err(err) = process_once(&state).await {
            error!(error = %err, "notifications scheduler iteration failed");
        }
    }
}
