use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use sqlx::Row;

/// Usage information for the current month for a tenant.
#[derive(Debug, Clone)]
pub struct UsageInfo {
    /// First day of the current month.
    pub month: NaiveDate,
    /// Number of events ingested this month (0 if no row exists yet).
    pub event_count: i64,
    /// Plan event limit (from the `tenants` table).
    pub event_limit: i64,
    /// Plan name (from the `tenants` table), e.g. "free", "pro".
    pub plan: String,
}

/// Atomically increment the event counter for `tenant_id` in the current month.
///
/// Uses an upsert so the row is created on first use. This is fire-and-forget —
/// callers should spawn this on a background task and never await the error.
pub async fn increment_usage(
    pool: &sqlx::PgPool,
    tenant_id: &str,
    count: i64,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO usage_counters (tenant_id, month, event_count)
           VALUES ($1, DATE_TRUNC('month', NOW())::DATE, $2)
           ON CONFLICT (tenant_id, month)
           DO UPDATE SET event_count = usage_counters.event_count + EXCLUDED.event_count"#,
    )
    .bind(tenant_id)
    .bind(count)
    .execute(pool)
    .await
    .map_err(|e| anyhow::anyhow!("increment_usage failed: {e}"))?;

    Ok(())
}

/// Fetch the current-month usage for `tenant_id`.
///
/// Joins `usage_counters` with `tenants` to get the plan event limit. Returns
/// `event_count = 0` when no row exists for this month yet.
///
/// `month` is read from PostgreSQL `DATE_TRUNC('month', NOW())` to avoid
/// clock-skew between the Rust process and the database at month boundaries.
pub async fn get_usage(pool: &sqlx::PgPool, tenant_id: &str) -> Result<UsageInfo> {
    // Left join so we get a row even if no events have been counted yet.
    let row = sqlx::query(
        r#"SELECT
               DATE_TRUNC('month', NOW())::DATE AS month,
               COALESCE(uc.event_count, 0) AS event_count,
               COALESCE(t.event_limit, 10000) AS event_limit,
               COALESCE(t.plan, 'free') AS plan
           FROM tenants t
           LEFT JOIN usage_counters uc
             ON uc.tenant_id = t.id
            AND uc.month = DATE_TRUNC('month', NOW())::DATE
           WHERE t.id = $1"#,
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| anyhow::anyhow!("get_usage query failed: {e}"))?;

    match row {
        Some(r) => Ok(UsageInfo {
            month: r.get::<NaiveDate, _>("month"),
            event_count: r.get::<i64, _>("event_count"),
            event_limit: r.get::<i64, _>("event_limit"),
            plan: r.get::<String, _>("plan"),
        }),
        None => {
            // Tenant row not found — return zero usage with default limits.
            let today = chrono::Utc::now().date_naive();
            let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
                .unwrap_or(today);
            Ok(UsageInfo {
                month: month_start,
                event_count: 0,
                event_limit: 10_000,
                plan: "free".to_string(),
            })
        }
    }
}
