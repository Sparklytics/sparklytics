use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Months, NaiveDate, Utc};
use rand::Rng;
use sparklytics_core::analytics::{
    AlertConditionType, AlertMetric, AlertRule, CreateAlertRuleRequest,
    CreateReportSubscriptionRequest, NotificationChannel, NotificationDelivery,
    NotificationDeliveryStatus, NotificationSourceType, ReportSubscription, SubscriptionSchedule,
    UpdateAlertRuleRequest, UpdateReportSubscriptionRequest,
};

use crate::DuckDbBackend;

fn random_alnum(len: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect()
}

fn generate_subscription_id() -> String {
    format!("sub_{}", random_alnum(21))
}

fn generate_alert_id() -> String {
    format!("alr_{}", random_alnum(21))
}

fn generate_delivery_id() -> String {
    format!("ntf_{}", random_alnum(21))
}

fn schedule_to_str(schedule: &SubscriptionSchedule) -> &'static str {
    match schedule {
        SubscriptionSchedule::Daily => "daily",
        SubscriptionSchedule::Weekly => "weekly",
        SubscriptionSchedule::Monthly => "monthly",
    }
}

fn schedule_from_str(raw: &str) -> Result<SubscriptionSchedule> {
    match raw {
        "daily" => Ok(SubscriptionSchedule::Daily),
        "weekly" => Ok(SubscriptionSchedule::Weekly),
        "monthly" => Ok(SubscriptionSchedule::Monthly),
        _ => Err(anyhow!("invalid schedule: {raw}")),
    }
}

fn channel_to_str(channel: &NotificationChannel) -> &'static str {
    match channel {
        NotificationChannel::Email => "email",
        NotificationChannel::Webhook => "webhook",
    }
}

fn channel_from_str(raw: &str) -> Result<NotificationChannel> {
    match raw {
        "email" => Ok(NotificationChannel::Email),
        "webhook" => Ok(NotificationChannel::Webhook),
        _ => Err(anyhow!("invalid channel: {raw}")),
    }
}

fn alert_metric_to_str(metric: &AlertMetric) -> &'static str {
    match metric {
        AlertMetric::Pageviews => "pageviews",
        AlertMetric::Visitors => "visitors",
        AlertMetric::Conversions => "conversions",
        AlertMetric::ConversionRate => "conversion_rate",
    }
}

fn alert_metric_from_str(raw: &str) -> Result<AlertMetric> {
    match raw {
        "pageviews" => Ok(AlertMetric::Pageviews),
        "visitors" => Ok(AlertMetric::Visitors),
        "conversions" => Ok(AlertMetric::Conversions),
        "conversion_rate" => Ok(AlertMetric::ConversionRate),
        _ => Err(anyhow!("invalid alert metric: {raw}")),
    }
}

fn condition_to_str(condition: &AlertConditionType) -> &'static str {
    match condition {
        AlertConditionType::Spike => "spike",
        AlertConditionType::Drop => "drop",
        AlertConditionType::ThresholdAbove => "threshold_above",
        AlertConditionType::ThresholdBelow => "threshold_below",
    }
}

fn condition_from_str(raw: &str) -> Result<AlertConditionType> {
    match raw {
        "spike" => Ok(AlertConditionType::Spike),
        "drop" => Ok(AlertConditionType::Drop),
        "threshold_above" => Ok(AlertConditionType::ThresholdAbove),
        "threshold_below" => Ok(AlertConditionType::ThresholdBelow),
        _ => Err(anyhow!("invalid alert condition: {raw}")),
    }
}

fn source_type_to_str(source_type: &NotificationSourceType) -> &'static str {
    match source_type {
        NotificationSourceType::Subscription => "subscription",
        NotificationSourceType::Alert => "alert",
    }
}

fn source_type_from_str(raw: &str) -> Result<NotificationSourceType> {
    match raw {
        "subscription" => Ok(NotificationSourceType::Subscription),
        "alert" => Ok(NotificationSourceType::Alert),
        _ => Err(anyhow!("invalid source_type: {raw}")),
    }
}

fn status_to_str(status: &NotificationDeliveryStatus) -> &'static str {
    match status {
        NotificationDeliveryStatus::Sent => "sent",
        NotificationDeliveryStatus::Failed => "failed",
    }
}

fn status_from_str(raw: &str) -> Result<NotificationDeliveryStatus> {
    match raw {
        "sent" => Ok(NotificationDeliveryStatus::Sent),
        "failed" => Ok(NotificationDeliveryStatus::Failed),
        _ => Err(anyhow!("invalid delivery status: {raw}")),
    }
}

pub fn compute_next_run_at(
    schedule: &SubscriptionSchedule,
    timezone: &str,
    from: DateTime<Utc>,
) -> Result<DateTime<Utc>> {
    let tz = timezone
        .parse::<chrono_tz::Tz>()
        .map_err(|_| anyhow!("invalid timezone"))?;
    let from_local = from.with_timezone(&tz);
    let next_local = match schedule {
        SubscriptionSchedule::Daily => from_local + Duration::days(1),
        SubscriptionSchedule::Weekly => from_local + Duration::days(7),
        SubscriptionSchedule::Monthly => from_local
            .checked_add_months(Months::new(1))
            .ok_or_else(|| anyhow!("monthly schedule overflow"))?,
    };
    Ok(next_local.with_timezone(&Utc))
}

fn map_report_subscription_row(row: &duckdb::Row<'_>) -> Result<ReportSubscription, duckdb::Error> {
    let schedule_raw: String = row.get(3)?;
    let channel_raw: String = row.get(5)?;
    let schedule = schedule_from_str(&schedule_raw).map_err(|e| {
        duckdb::Error::FromSqlConversionFailure(
            3,
            duckdb::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })?;
    let channel = channel_from_str(&channel_raw).map_err(|e| {
        duckdb::Error::FromSqlConversionFailure(
            5,
            duckdb::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })?;
    Ok(ReportSubscription {
        id: row.get(0)?,
        website_id: row.get(1)?,
        report_id: row.get(2)?,
        schedule,
        timezone: row.get(4)?,
        channel,
        target: row.get(6)?,
        is_active: row.get(7)?,
        last_run_at: row.get(8)?,
        next_run_at: row.get(9)?,
        created_at: row.get(10)?,
    })
}

fn map_alert_rule_row(row: &duckdb::Row<'_>) -> Result<AlertRule, duckdb::Error> {
    let metric_raw: String = row.get(3)?;
    let condition_raw: String = row.get(4)?;
    let channel_raw: String = row.get(7)?;
    let metric = alert_metric_from_str(&metric_raw).map_err(|e| {
        duckdb::Error::FromSqlConversionFailure(
            3,
            duckdb::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })?;
    let condition_type = condition_from_str(&condition_raw).map_err(|e| {
        duckdb::Error::FromSqlConversionFailure(
            4,
            duckdb::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })?;
    let channel = channel_from_str(&channel_raw).map_err(|e| {
        duckdb::Error::FromSqlConversionFailure(
            7,
            duckdb::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })?;
    Ok(AlertRule {
        id: row.get(0)?,
        website_id: row.get(1)?,
        name: row.get(2)?,
        metric,
        condition_type,
        threshold_value: row.get(5)?,
        lookback_days: row.get(6)?,
        channel,
        target: row.get(8)?,
        is_active: row.get(9)?,
        created_at: row.get(10)?,
    })
}

fn map_notification_delivery_row(
    row: &duckdb::Row<'_>,
) -> Result<NotificationDelivery, duckdb::Error> {
    let source_type_raw: String = row.get(1)?;
    let status_raw: String = row.get(4)?;
    let source_type = source_type_from_str(&source_type_raw).map_err(|e| {
        duckdb::Error::FromSqlConversionFailure(
            1,
            duckdb::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })?;
    let status = status_from_str(&status_raw).map_err(|e| {
        duckdb::Error::FromSqlConversionFailure(
            4,
            duckdb::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })?;
    Ok(NotificationDelivery {
        id: row.get(0)?,
        source_type,
        source_id: row.get(2)?,
        idempotency_key: row.get(3)?,
        status,
        error_message: row.get(5)?,
        delivered_at: row.get(6)?,
    })
}

impl DuckDbBackend {
    pub async fn list_report_subscriptions(
        &self,
        website_id: &str,
    ) -> Result<Vec<ReportSubscription>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id,
                website_id,
                report_id,
                schedule,
                timezone,
                channel,
                target,
                is_active,
                CAST(last_run_at AS VARCHAR),
                CAST(next_run_at AS VARCHAR),
                CAST(created_at AS VARCHAR)
            FROM report_subscriptions
            WHERE website_id = ?1
            ORDER BY created_at DESC, id DESC
            "#,
        )?;
        let mut out = Vec::new();
        for row in stmt.query_map(duckdb::params![website_id], map_report_subscription_row)? {
            out.push(row?);
        }
        Ok(out)
    }

    pub async fn get_report_subscription(
        &self,
        website_id: &str,
        subscription_id: &str,
    ) -> Result<Option<ReportSubscription>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id,
                website_id,
                report_id,
                schedule,
                timezone,
                channel,
                target,
                is_active,
                CAST(last_run_at AS VARCHAR),
                CAST(next_run_at AS VARCHAR),
                CAST(created_at AS VARCHAR)
            FROM report_subscriptions
            WHERE website_id = ?1 AND id = ?2
            "#,
        )?;
        Ok(stmt
            .query_row(
                duckdb::params![website_id, subscription_id],
                map_report_subscription_row,
            )
            .ok())
    }

    pub async fn create_report_subscription(
        &self,
        website_id: &str,
        req: CreateReportSubscriptionRequest,
    ) -> Result<ReportSubscription> {
        let id = generate_subscription_id();
        let timezone = req.timezone.unwrap_or_else(|| "UTC".to_string());
        let now = Utc::now();
        let next_run = compute_next_run_at(&req.schedule, &timezone, now)?;
        let conn = self.conn.lock().await;
        conn.execute(
            r#"
            INSERT INTO report_subscriptions (
                id, website_id, report_id, schedule, timezone, channel, target, is_active, next_run_at, created_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, TRUE, CAST(?8 AS TIMESTAMP), CURRENT_TIMESTAMP
            )
            "#,
            duckdb::params![
                id,
                website_id,
                req.report_id,
                schedule_to_str(&req.schedule),
                timezone,
                channel_to_str(&req.channel),
                req.target,
                next_run.to_rfc3339(),
            ],
        )?;
        drop(conn);
        self.get_report_subscription(website_id, &id)
            .await?
            .ok_or_else(|| anyhow!("subscription not found after create"))
    }

    pub async fn update_report_subscription(
        &self,
        website_id: &str,
        subscription_id: &str,
        req: UpdateReportSubscriptionRequest,
    ) -> Result<Option<ReportSubscription>> {
        let Some(existing) = self
            .get_report_subscription(website_id, subscription_id)
            .await?
        else {
            return Ok(None);
        };
        let report_id = req.report_id.unwrap_or(existing.report_id);
        let schedule = req.schedule.unwrap_or(existing.schedule);
        let timezone = req
            .timezone
            .unwrap_or(Some(existing.timezone))
            .unwrap_or_else(|| "UTC".to_string());
        let channel = req.channel.unwrap_or(existing.channel);
        let target = req
            .target
            .unwrap_or(Some(existing.target))
            .unwrap_or_default();
        let is_active = req.is_active.unwrap_or(existing.is_active);
        let next_run = compute_next_run_at(&schedule, &timezone, Utc::now())?;

        let conn = self.conn.lock().await;
        conn.execute(
            r#"
            UPDATE report_subscriptions
            SET report_id = ?1,
                schedule = ?2,
                timezone = ?3,
                channel = ?4,
                target = ?5,
                is_active = ?6,
                next_run_at = CAST(?7 AS TIMESTAMP)
            WHERE website_id = ?8 AND id = ?9
            "#,
            duckdb::params![
                report_id,
                schedule_to_str(&schedule),
                timezone,
                channel_to_str(&channel),
                target,
                is_active,
                next_run.to_rfc3339(),
                website_id,
                subscription_id
            ],
        )?;
        drop(conn);
        self.get_report_subscription(website_id, subscription_id)
            .await
    }

    pub async fn delete_report_subscription(
        &self,
        website_id: &str,
        subscription_id: &str,
    ) -> Result<bool> {
        let conn = self.conn.lock().await;
        let rows = conn.execute(
            "DELETE FROM report_subscriptions WHERE website_id = ?1 AND id = ?2",
            duckdb::params![website_id, subscription_id],
        )?;
        Ok(rows > 0)
    }

    pub async fn list_due_report_subscriptions(
        &self,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ReportSubscription>> {
        let bounded_limit = limit.clamp(1, 200);
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id,
                website_id,
                report_id,
                schedule,
                timezone,
                channel,
                target,
                is_active,
                CAST(last_run_at AS VARCHAR),
                CAST(next_run_at AS VARCHAR),
                CAST(created_at AS VARCHAR)
            FROM report_subscriptions
            WHERE is_active = TRUE
              AND next_run_at <= CAST(?1 AS TIMESTAMP)
            ORDER BY next_run_at ASC
            LIMIT ?2
            "#,
        )?;
        let mut out = Vec::new();
        for row in stmt.query_map(
            duckdb::params![now.to_rfc3339(), bounded_limit],
            map_report_subscription_row,
        )? {
            out.push(row?);
        }
        Ok(out)
    }

    pub async fn mark_report_subscription_ran(
        &self,
        subscription_id: &str,
        ran_at: DateTime<Utc>,
        schedule: &SubscriptionSchedule,
        timezone: &str,
    ) -> Result<()> {
        let next_run = compute_next_run_at(schedule, timezone, ran_at)?;
        let conn = self.conn.lock().await;
        conn.execute(
            r#"
            UPDATE report_subscriptions
            SET last_run_at = CAST(?1 AS TIMESTAMP),
                next_run_at = CAST(?2 AS TIMESTAMP)
            WHERE id = ?3
            "#,
            duckdb::params![ran_at.to_rfc3339(), next_run.to_rfc3339(), subscription_id],
        )?;
        Ok(())
    }

    pub async fn set_report_subscription_next_run_at(
        &self,
        subscription_id: &str,
        next_run_at: DateTime<Utc>,
    ) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE report_subscriptions SET next_run_at = CAST(?1 AS TIMESTAMP) WHERE id = ?2",
            duckdb::params![next_run_at.to_rfc3339(), subscription_id],
        )?;
        Ok(())
    }

    pub async fn list_alert_rules(&self, website_id: &str) -> Result<Vec<AlertRule>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id, website_id, name, metric, condition_type,
                threshold_value, lookback_days, channel, target,
                is_active, CAST(created_at AS VARCHAR)
            FROM alert_rules
            WHERE website_id = ?1
            ORDER BY created_at DESC, id DESC
            "#,
        )?;
        let mut out = Vec::new();
        for row in stmt.query_map(duckdb::params![website_id], map_alert_rule_row)? {
            out.push(row?);
        }
        Ok(out)
    }

    pub async fn get_alert_rule(
        &self,
        website_id: &str,
        alert_id: &str,
    ) -> Result<Option<AlertRule>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id, website_id, name, metric, condition_type,
                threshold_value, lookback_days, channel, target,
                is_active, CAST(created_at AS VARCHAR)
            FROM alert_rules
            WHERE website_id = ?1 AND id = ?2
            "#,
        )?;
        Ok(stmt
            .query_row(duckdb::params![website_id, alert_id], map_alert_rule_row)
            .ok())
    }

    pub async fn create_alert_rule(
        &self,
        website_id: &str,
        req: CreateAlertRuleRequest,
    ) -> Result<AlertRule> {
        let id = generate_alert_id();
        let lookback_days = req.lookback_days.unwrap_or(7).clamp(1, 30);
        let conn = self.conn.lock().await;
        conn.execute(
            r#"
            INSERT INTO alert_rules (
                id, website_id, name, metric, condition_type, threshold_value,
                lookback_days, channel, target, is_active, created_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, TRUE, CURRENT_TIMESTAMP
            )
            "#,
            duckdb::params![
                id,
                website_id,
                req.name,
                alert_metric_to_str(&req.metric),
                condition_to_str(&req.condition_type),
                req.threshold_value,
                lookback_days,
                channel_to_str(&req.channel),
                req.target
            ],
        )?;
        drop(conn);
        self.get_alert_rule(website_id, &id)
            .await?
            .ok_or_else(|| anyhow!("alert rule not found after create"))
    }

    pub async fn update_alert_rule(
        &self,
        website_id: &str,
        alert_id: &str,
        req: UpdateAlertRuleRequest,
    ) -> Result<Option<AlertRule>> {
        let Some(existing) = self.get_alert_rule(website_id, alert_id).await? else {
            return Ok(None);
        };

        let name = req.name.unwrap_or(existing.name);
        let metric = req.metric.unwrap_or(existing.metric);
        let condition_type = req.condition_type.unwrap_or(existing.condition_type);
        let threshold_value = req.threshold_value.unwrap_or(existing.threshold_value);
        let lookback_days = req
            .lookback_days
            .unwrap_or(existing.lookback_days)
            .clamp(1, 30);
        let channel = req.channel.unwrap_or(existing.channel);
        let target = req
            .target
            .unwrap_or(Some(existing.target))
            .unwrap_or_default();
        let is_active = req.is_active.unwrap_or(existing.is_active);

        let conn = self.conn.lock().await;
        conn.execute(
            r#"
            UPDATE alert_rules
            SET name = ?1,
                metric = ?2,
                condition_type = ?3,
                threshold_value = ?4,
                lookback_days = ?5,
                channel = ?6,
                target = ?7,
                is_active = ?8
            WHERE website_id = ?9 AND id = ?10
            "#,
            duckdb::params![
                name,
                alert_metric_to_str(&metric),
                condition_to_str(&condition_type),
                threshold_value,
                lookback_days,
                channel_to_str(&channel),
                target,
                is_active,
                website_id,
                alert_id,
            ],
        )?;
        drop(conn);
        self.get_alert_rule(website_id, alert_id).await
    }

    pub async fn delete_alert_rule(&self, website_id: &str, alert_id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let rows = conn.execute(
            "DELETE FROM alert_rules WHERE website_id = ?1 AND id = ?2",
            duckdb::params![website_id, alert_id],
        )?;
        Ok(rows > 0)
    }

    pub async fn list_active_alert_rules(&self, limit: i64) -> Result<Vec<AlertRule>> {
        let bounded_limit = limit.clamp(1, 1000);
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id, website_id, name, metric, condition_type,
                threshold_value, lookback_days, channel, target,
                is_active, CAST(created_at AS VARCHAR)
            FROM alert_rules
            WHERE is_active = TRUE
            ORDER BY created_at ASC
            LIMIT ?1
            "#,
        )?;
        let mut out = Vec::new();
        for row in stmt.query_map(duckdb::params![bounded_limit], map_alert_rule_row)? {
            out.push(row?);
        }
        Ok(out)
    }

    pub async fn get_daily_alert_metric_series(
        &self,
        website_id: &str,
        metric: &AlertMetric,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<(NaiveDate, f64)>> {
        let start = start_date
            .and_hms_opt(0, 0, 0)
            .expect("valid start")
            .and_utc()
            .to_rfc3339();
        let end_exclusive = (end_date + Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .expect("valid end")
            .and_utc()
            .to_rfc3339();
        let conn = self.conn.lock().await;

        let sql = match metric {
            AlertMetric::Pageviews => {
                r#"
                SELECT
                    CAST(CAST(date_trunc('day', created_at) AS DATE) AS VARCHAR) AS period_day,
                    CAST(COUNT(*) AS DOUBLE) AS metric_value
                FROM events
                WHERE website_id = ?1
                  AND created_at >= CAST(?2 AS TIMESTAMP)
                  AND created_at < CAST(?3 AS TIMESTAMP)
                  AND event_type = 'pageview'
                GROUP BY period_day
                ORDER BY period_day
                "#
            }
            AlertMetric::Visitors => {
                r#"
                SELECT
                    CAST(CAST(date_trunc('day', created_at) AS DATE) AS VARCHAR) AS period_day,
                    CAST(COUNT(DISTINCT visitor_id) AS DOUBLE) AS metric_value
                FROM events
                WHERE website_id = ?1
                  AND created_at >= CAST(?2 AS TIMESTAMP)
                  AND created_at < CAST(?3 AS TIMESTAMP)
                GROUP BY period_day
                ORDER BY period_day
                "#
            }
            AlertMetric::Conversions => {
                r#"
                SELECT
                    CAST(CAST(date_trunc('day', created_at) AS DATE) AS VARCHAR) AS period_day,
                    CAST(COUNT(*) AS DOUBLE) AS metric_value
                FROM events
                WHERE website_id = ?1
                  AND created_at >= CAST(?2 AS TIMESTAMP)
                  AND created_at < CAST(?3 AS TIMESTAMP)
                  AND event_name = 'goal_conversion'
                GROUP BY period_day
                ORDER BY period_day
                "#
            }
            AlertMetric::ConversionRate => {
                r#"
                WITH conv AS (
                    SELECT
                        CAST(date_trunc('day', created_at) AS DATE) AS period_day,
                        CAST(COUNT(*) AS DOUBLE) AS conversions
                    FROM events
                    WHERE website_id = ?1
                      AND created_at >= CAST(?2 AS TIMESTAMP)
                      AND created_at < CAST(?3 AS TIMESTAMP)
                      AND event_name = 'goal_conversion'
                    GROUP BY period_day
                ),
                sess AS (
                    SELECT
                        CAST(date_trunc('day', first_seen) AS DATE) AS period_day,
                        CAST(COUNT(*) AS DOUBLE) AS sessions
                    FROM sessions
                    WHERE website_id = ?1
                      AND first_seen >= CAST(?2 AS TIMESTAMP)
                      AND first_seen < CAST(?3 AS TIMESTAMP)
                    GROUP BY period_day
                )
                SELECT
                    CAST(COALESCE(conv.period_day, sess.period_day) AS VARCHAR) AS period_day,
                    CASE
                        WHEN COALESCE(sess.sessions, 0) = 0 THEN 0.0
                        ELSE (COALESCE(conv.conversions, 0) / sess.sessions) * 100.0
                    END AS metric_value
                FROM conv
                FULL OUTER JOIN sess ON conv.period_day = sess.period_day
                ORDER BY period_day
                "#
            }
        };

        let mut stmt = conn.prepare(sql)?;
        let mut out = Vec::new();
        for row in stmt.query_map(duckdb::params![website_id, start, end_exclusive], |row| {
            let day: String = row.get(0)?;
            let value: f64 = row.get(1)?;
            Ok((day, value))
        })? {
            let (day_raw, value) = row?;
            let day = NaiveDate::parse_from_str(&day_raw, "%Y-%m-%d")
                .map_err(|e| anyhow!("invalid date in alert metric series: {e}"))?;
            out.push((day, value));
        }
        Ok(out)
    }

    pub async fn list_notification_deliveries_for_website(
        &self,
        website_id: &str,
        limit: i64,
    ) -> Result<Vec<NotificationDelivery>> {
        let bounded_limit = limit.clamp(1, 200);
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                d.id,
                d.source_type,
                d.source_id,
                d.idempotency_key,
                d.status,
                d.error_message,
                CAST(d.delivered_at AS VARCHAR)
            FROM notification_deliveries d
            LEFT JOIN report_subscriptions s
              ON d.source_type = 'subscription' AND d.source_id = s.id
            LEFT JOIN alert_rules a
              ON d.source_type = 'alert' AND d.source_id = a.id
            WHERE s.website_id = ?1 OR a.website_id = ?1
            ORDER BY d.delivered_at DESC, d.id DESC
            LIMIT ?2
            "#,
        )?;
        let mut out = Vec::new();
        for row in stmt.query_map(
            duckdb::params![website_id, bounded_limit],
            map_notification_delivery_row,
        )? {
            out.push(row?);
        }
        Ok(out)
    }

    pub async fn has_notification_delivery(&self, idempotency_key: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let exists: i64 = conn
            .prepare("SELECT COUNT(*) FROM notification_deliveries WHERE idempotency_key = ?1")?
            .query_row(duckdb::params![idempotency_key], |row| row.get(0))?;
        Ok(exists > 0)
    }

    pub async fn create_notification_delivery(
        &self,
        source_type: NotificationSourceType,
        source_id: &str,
        idempotency_key: &str,
        status: NotificationDeliveryStatus,
        error_message: Option<&str>,
    ) -> Result<NotificationDelivery> {
        let id = generate_delivery_id();
        let conn = self.conn.lock().await;
        conn.execute(
            r#"
            INSERT INTO notification_deliveries (
                id,
                source_type,
                source_id,
                idempotency_key,
                status,
                error_message,
                delivered_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP
            )
            "#,
            duckdb::params![
                id,
                source_type_to_str(&source_type),
                source_id,
                idempotency_key,
                status_to_str(&status),
                error_message,
            ],
        )?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id,
                source_type,
                source_id,
                idempotency_key,
                status,
                error_message,
                CAST(delivered_at AS VARCHAR)
            FROM notification_deliveries
            WHERE id = ?1
            "#,
        )?;
        Ok(stmt.query_row(duckdb::params![id], map_notification_delivery_row)?)
    }

    pub async fn count_goal_conversions_for_range(
        &self,
        website_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64> {
        let conn = self.conn.lock().await;
        let count: i64 = conn
            .prepare(
                r#"
                SELECT COUNT(*)
                FROM events
                WHERE website_id = ?1
                  AND event_name = 'goal_conversion'
                  AND created_at >= CAST(?2 AS TIMESTAMP)
                  AND created_at < CAST(?3 AS TIMESTAMP)
                "#,
            )?
            .query_row(
                duckdb::params![website_id, start.to_rfc3339(), end.to_rfc3339()],
                |row| row.get(0),
            )?;
        Ok(count)
    }
}
