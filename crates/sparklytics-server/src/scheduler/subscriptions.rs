use std::sync::Arc;

use chrono::Utc;
use serde_json::json;
use sparklytics_core::analytics::{
    AnalyticsBackend, NotificationDeliveryStatus, NotificationSourceType,
};

use crate::{
    routes::reports::execute_report_config_with_backend,
    state::AppState,
};

use super::delivery::deliver_and_record;

fn max_subscriptions_per_tick() -> i64 {
    std::env::var("SPARKLYTICS_SCHEDULER_MAX_SUBSCRIPTIONS_PER_TICK")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .map(|v| v.clamp(1, 100))
        .unwrap_or(10)
}

fn idempotency_bucket(next_run_at: &str) -> String {
    next_run_at
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(16)
        .collect::<String>()
}

pub async fn run_due_subscriptions(state: &Arc<AppState>) -> anyhow::Result<usize> {
    let due = state
        .scheduler_db
        .list_due_report_subscriptions(Utc::now(), max_subscriptions_per_tick())
        .await?;
    let mut runs = 0usize;

    for subscription in due {
        let idempotency_key = format!(
            "sub:{}:{}",
            subscription.id,
            idempotency_bucket(&subscription.next_run_at)
        );
        let now = Utc::now();

        let report = state
            .scheduler_db
            .get_report(&subscription.website_id, None, &subscription.report_id)
            .await?;
        let Some(report) = report else {
            if !state
                .scheduler_db
                .has_notification_delivery(&idempotency_key)
                .await?
            {
                state
                    .scheduler_db
                    .create_notification_delivery(
                        NotificationSourceType::Subscription,
                        &subscription.id,
                        &idempotency_key,
                        NotificationDeliveryStatus::Failed,
                        Some("report not found"),
                    )
                    .await?;
                state
                    .scheduler_db
                    .mark_report_subscription_ran(
                        &subscription.id,
                        now,
                        &subscription.schedule,
                        &subscription.timezone,
                    )
                    .await?;
                runs += 1;
            }
            continue;
        };

        let report_data = match execute_report_config_with_backend(
            state.scheduler_db.as_ref(),
            &subscription.website_id,
            &report.config,
        )
        .await
        {
            Ok(data) => data,
            Err(err) => {
                if !state
                    .scheduler_db
                    .has_notification_delivery(&idempotency_key)
                    .await?
                {
                    state
                        .scheduler_db
                        .create_notification_delivery(
                            NotificationSourceType::Subscription,
                            &subscription.id,
                            &idempotency_key,
                            NotificationDeliveryStatus::Failed,
                            Some(&err.to_string()),
                        )
                        .await?;
                    state
                        .scheduler_db
                        .mark_report_subscription_ran(
                            &subscription.id,
                            now,
                            &subscription.schedule,
                            &subscription.timezone,
                        )
                        .await?;
                    runs += 1;
                }
                continue;
            }
        };

        let payload = json!({
            "kind": "report_subscription",
            "website_id": subscription.website_id,
            "subscription_id": subscription.id,
            "report_id": subscription.report_id,
            "generated_at": now.to_rfc3339(),
            "data": report_data
        });

        let delivered = deliver_and_record(
            state,
            NotificationSourceType::Subscription,
            &subscription.id,
            &idempotency_key,
            subscription.channel,
            subscription.target,
            payload,
        )
        .await?;

        if delivered.is_some() {
            state
                .scheduler_db
                .mark_report_subscription_ran(
                    &subscription.id,
                    now,
                    &subscription.schedule,
                    &subscription.timezone,
                )
                .await?;
            runs += 1;
        }
    }

    Ok(runs)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{Duration, Utc};
    use sparklytics_core::{
        analytics::{
            CreateReportSubscriptionRequest, NotificationChannel, NotificationDeliveryStatus,
            SubscriptionSchedule,
        },
        config::{AppMode, AuthMode, Config},
    };
    use sparklytics_duckdb::DuckDbBackend;

    use crate::state::AppState;

    use super::run_due_subscriptions;

    fn unique_data_dir() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix time")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("sparklytics-subscriptions-{nanos}"))
            .to_string_lossy()
            .to_string()
    }

    fn test_config(data_dir: String) -> Config {
        Config {
            port: 0,
            data_dir,
            geoip_path: "./GeoLite2-City.mmdb".to_string(),
            auth_mode: AuthMode::None,
            https: false,
            retention_days: 365,
            cors_origins: vec![],
            session_days: 7,
            buffer_flush_interval_ms: 5_000,
            buffer_max_size: 100,
            mode: AppMode::SelfHosted,
            argon2_memory_kb: 65_536,
            public_url: "http://localhost:3000".to_string(),
            rate_limit_disable: false,
            duckdb_memory_limit: "1GB".to_string(),
        }
    }

    #[tokio::test]
    async fn missing_report_records_failed_delivery_once_and_advances_schedule() {
        let data_dir = unique_data_dir();
        std::fs::create_dir_all(&data_dir).expect("create temp dir");
        let db_path = format!("{data_dir}/sparklytics.db");
        let db = DuckDbBackend::open(&db_path, "1GB").expect("open db");
        let state = Arc::new(AppState::new(db, test_config(data_dir)));

        let website_id = "site_sched";
        let subscription = state
            .db
            .create_report_subscription(
                website_id,
                CreateReportSubscriptionRequest {
                    report_id: "report_missing".to_string(),
                    schedule: SubscriptionSchedule::Daily,
                    timezone: Some("UTC".to_string()),
                    channel: NotificationChannel::Email,
                    target: "ops@example.com".to_string(),
                },
            )
            .await
            .expect("create subscription");
        state
            .db
            .set_report_subscription_next_run_at(
                &subscription.id,
                Utc::now() - Duration::minutes(1),
            )
            .await
            .expect("set due");

        let first_runs = run_due_subscriptions(&state)
            .await
            .expect("first scheduler run");
        let second_runs = run_due_subscriptions(&state)
            .await
            .expect("second scheduler run");
        assert_eq!(first_runs, 1, "first run should process due subscription");
        assert_eq!(second_runs, 0, "second run should not duplicate delivery");

        let history = state
            .db
            .list_notification_deliveries_for_website(website_id, 50)
            .await
            .expect("history");
        assert_eq!(history.len(), 1, "should keep a single delivery row");
        assert!(matches!(
            history[0].status,
            NotificationDeliveryStatus::Failed
        ));
    }
}
