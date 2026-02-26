use std::{collections::HashMap, sync::Arc};

use chrono::{Duration, NaiveDate, Utc};
use serde_json::json;
use sparklytics_core::analytics::{AlertConditionType, NotificationSourceType};

use crate::state::AppState;

use super::delivery::deliver_and_record;

fn max_alert_rules_per_tick() -> i64 {
    std::env::var("SPARKLYTICS_SCHEDULER_MAX_ALERTS_PER_TICK")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .map(|v| v.clamp(1, 200))
        .unwrap_or(25)
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

fn stddev(values: &[f64], mean_value: f64) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let variance = values
        .iter()
        .map(|v| {
            let diff = *v - mean_value;
            diff * diff
        })
        .sum::<f64>()
        / values.len() as f64;
    Some(variance.sqrt())
}

pub async fn run_alert_checks(state: &Arc<AppState>) -> anyhow::Result<usize> {
    let rules = state
        .scheduler_db
        .list_active_alert_rules(max_alert_rules_per_tick())
        .await?;
    let today = Utc::now().date_naive();
    let mut deliveries = 0usize;

    for rule in rules {
        let baseline_start = today - Duration::days(rule.lookback_days);
        let series = state
            .scheduler_db
            .get_daily_alert_metric_series(&rule.website_id, &rule.metric, baseline_start, today)
            .await?;
        let daily_map: HashMap<NaiveDate, f64> = series.into_iter().collect();
        let current_value = daily_map.get(&today).copied().unwrap_or(0.0);
        let (triggered, baseline_mean, baseline_stddev, z_score) = match rule.condition_type {
            AlertConditionType::ThresholdAbove => {
                (current_value >= rule.threshold_value, None, None, None)
            }
            AlertConditionType::ThresholdBelow => {
                (current_value <= rule.threshold_value, None, None, None)
            }
            AlertConditionType::Spike | AlertConditionType::Drop => {
                let mut baseline = Vec::with_capacity(rule.lookback_days as usize);
                for offset in 1..=rule.lookback_days {
                    let day = today - Duration::days(offset);
                    baseline.push(daily_map.get(&day).copied().unwrap_or(0.0));
                }
                let Some(baseline_mean) = mean(&baseline) else {
                    continue;
                };
                let baseline_stddev = stddev(&baseline, baseline_mean).unwrap_or(0.0);
                if baseline_stddev <= f64::EPSILON {
                    continue;
                }
                let z = (current_value - baseline_mean) / baseline_stddev;
                let triggered = match rule.condition_type {
                    AlertConditionType::Spike => z >= rule.threshold_value,
                    AlertConditionType::Drop => z <= -rule.threshold_value,
                    _ => false,
                };
                (
                    triggered,
                    Some(baseline_mean),
                    Some(baseline_stddev),
                    Some(z),
                )
            }
        };

        if !triggered {
            continue;
        }

        let idempotency_key = format!("alert:{}:{}", rule.id, today.format("%Y%m%d"));
        let payload = json!({
            "kind": "alert",
            "website_id": rule.website_id,
            "alert_id": rule.id,
            "name": rule.name,
            "metric": rule.metric,
            "condition_type": rule.condition_type,
            "threshold_value": rule.threshold_value,
            "current_value": current_value,
            "baseline_mean": baseline_mean,
            "baseline_stddev": baseline_stddev,
            "z_score": z_score,
            "triggered_at": Utc::now().to_rfc3339(),
        });

        let delivered = deliver_and_record(
            state,
            NotificationSourceType::Alert,
            &rule.id,
            &idempotency_key,
            rule.channel,
            rule.target,
            payload,
        )
        .await?;
        if delivered.is_some() {
            deliveries += 1;
        }
    }

    Ok(deliveries)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{Duration, Utc};
    use sparklytics_core::{
        analytics::{
            AlertConditionType, AlertMetric, CreateAlertRuleRequest, NotificationChannel,
            UpdateAlertRuleRequest,
        },
        config::{AppMode, AuthMode, Config},
        event::Event,
    };
    use sparklytics_duckdb::DuckDbBackend;

    use crate::state::AppState;

    use super::run_alert_checks;

    fn unique_data_dir() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix time")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("sparklytics-alerts-{nanos}"))
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

    fn make_pageview(website_id: &str, day: chrono::NaiveDate, idx: i64) -> Event {
        Event {
            id: format!("evt-{}-{idx}", uuid::Uuid::new_v4()),
            website_id: website_id.to_string(),
            tenant_id: None,
            session_id: format!("sess-{website_id}-{day}-{idx}"),
            visitor_id: format!("visitor-{website_id}-{idx}"),
            event_type: "pageview".to_string(),
            url: "https://example.com/landing".to_string(),
            referrer_url: None,
            referrer_domain: None,
            event_name: None,
            event_data: None,
            country: None,
            region: None,
            city: None,
            browser: None,
            browser_version: None,
            os: None,
            os_version: None,
            device_type: None,
            screen: None,
            language: None,
            utm_source: None,
            utm_medium: None,
            utm_campaign: None,
            utm_term: None,
            utm_content: None,
            link_id: None,
            pixel_id: None,
            source_ip: None,
            user_agent: None,
            is_bot: false,
            bot_score: 0,
            bot_reason: None,
            created_at: day.and_hms_opt(12, 0, 0).expect("valid noon").and_utc()
                + Duration::milliseconds(idx),
        }
    }

    async fn seed_pageviews(
        state: &Arc<AppState>,
        website_id: &str,
        day: chrono::NaiveDate,
        count: i64,
    ) {
        let mut events = Vec::with_capacity(count as usize);
        for idx in 0..count {
            events.push(make_pageview(website_id, day, idx));
        }
        state
            .db
            .insert_events(&events)
            .await
            .expect("insert events");
    }

    #[tokio::test]
    async fn evaluates_spike_drop_threshold_and_inactive_rules_with_idempotency() {
        let data_dir = unique_data_dir();
        std::fs::create_dir_all(&data_dir).expect("create temp dir");
        let db_path = format!("{data_dir}/sparklytics.db");
        let db = DuckDbBackend::open(&db_path, "1GB").expect("open db");
        let state = Arc::new(AppState::new(db, test_config(data_dir)));

        let spike_site = "site_spike";
        let drop_site = "site_drop";
        state
            .db
            .seed_website(spike_site, "spike.example.com")
            .await
            .expect("seed spike website");
        state
            .db
            .seed_website(drop_site, "drop.example.com")
            .await
            .expect("seed drop website");

        let today = Utc::now().date_naive();
        seed_pageviews(&state, spike_site, today - Duration::days(3), 1).await;
        seed_pageviews(&state, spike_site, today - Duration::days(2), 2).await;
        seed_pageviews(&state, spike_site, today - Duration::days(1), 3).await;
        seed_pageviews(&state, spike_site, today, 8).await;

        seed_pageviews(&state, drop_site, today - Duration::days(3), 8).await;
        seed_pageviews(&state, drop_site, today - Duration::days(2), 6).await;
        seed_pageviews(&state, drop_site, today - Duration::days(1), 4).await;
        seed_pageviews(&state, drop_site, today, 1).await;

        let _spike_rule = state
            .db
            .create_alert_rule(
                spike_site,
                CreateAlertRuleRequest {
                    name: "Traffic spike".to_string(),
                    metric: AlertMetric::Pageviews,
                    condition_type: AlertConditionType::Spike,
                    threshold_value: 2.0,
                    lookback_days: Some(3),
                    channel: NotificationChannel::Email,
                    target: "ops@example.com".to_string(),
                },
            )
            .await
            .expect("create spike rule");
        let _threshold_above_rule = state
            .db
            .create_alert_rule(
                spike_site,
                CreateAlertRuleRequest {
                    name: "High pageviews".to_string(),
                    metric: AlertMetric::Pageviews,
                    condition_type: AlertConditionType::ThresholdAbove,
                    threshold_value: 5.0,
                    lookback_days: Some(3),
                    channel: NotificationChannel::Email,
                    target: "ops@example.com".to_string(),
                },
            )
            .await
            .expect("create threshold_above rule");
        let threshold_below_rule = state
            .db
            .create_alert_rule(
                spike_site,
                CreateAlertRuleRequest {
                    name: "Low pageviews".to_string(),
                    metric: AlertMetric::Pageviews,
                    condition_type: AlertConditionType::ThresholdBelow,
                    threshold_value: 5.0,
                    lookback_days: Some(3),
                    channel: NotificationChannel::Email,
                    target: "ops@example.com".to_string(),
                },
            )
            .await
            .expect("create threshold_below rule");
        state
            .db
            .update_alert_rule(
                spike_site,
                &threshold_below_rule.id,
                UpdateAlertRuleRequest {
                    is_active: Some(false),
                    ..UpdateAlertRuleRequest::default()
                },
            )
            .await
            .expect("disable threshold_below rule");
        let _drop_rule = state
            .db
            .create_alert_rule(
                drop_site,
                CreateAlertRuleRequest {
                    name: "Traffic drop".to_string(),
                    metric: AlertMetric::Pageviews,
                    condition_type: AlertConditionType::Drop,
                    threshold_value: 2.0,
                    lookback_days: Some(3),
                    channel: NotificationChannel::Email,
                    target: "ops@example.com".to_string(),
                },
            )
            .await
            .expect("create drop rule");

        let first = run_alert_checks(&state).await.expect("first run");
        let second = run_alert_checks(&state).await.expect("second run");
        assert_eq!(
            first, 3,
            "spike + threshold_above + drop should trigger once each"
        );
        assert_eq!(second, 0, "second run should be idempotent for same day");

        let spike_history = state
            .db
            .list_notification_deliveries_for_website(spike_site, 20)
            .await
            .expect("spike history");
        let drop_history = state
            .db
            .list_notification_deliveries_for_website(drop_site, 20)
            .await
            .expect("drop history");
        assert_eq!(
            spike_history.len(),
            2,
            "only active triggered spike-site rules should be delivered"
        );
        assert_eq!(drop_history.len(), 1, "drop-site rule should trigger once");
    }
}
