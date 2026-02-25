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

pub async fn run_scheduler_loop(state: Arc<AppState>) {
    let tick = scheduler_tick_seconds();
    info!(tick_seconds = tick, "Notifications scheduler started");
    let mut interval = tokio::time::interval(Duration::from_secs(tick));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        interval.tick().await;
        if let Err(err) = process_once(&state).await {
            error!(error = %err, "notifications scheduler iteration failed");
        }
    }
}
