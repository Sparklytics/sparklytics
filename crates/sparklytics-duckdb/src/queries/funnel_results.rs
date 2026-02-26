use anyhow::{anyhow, Result};
use chrono::{LocalResult, TimeZone};
use chrono_tz::Tz;

use sparklytics_core::analytics::{
    AnalyticsFilter, Funnel, FunnelResults, FunnelStep, FunnelStepResult, MatchOperator, StepType,
};

use crate::queries::bot_filters::append_event_bot_filter;
use crate::DuckDbBackend;

use super::funnels::get_funnel_inner;

fn append_event_filters(
    filter: &AnalyticsFilter,
    filter_sql: &mut String,
    params: &mut Vec<Box<dyn duckdb::types::ToSql>>,
    param_idx: &mut usize,
) {
    append_event_bot_filter(filter_sql, filter.include_bots, "e.");
    if let Some(ref country) = filter.filter_country {
        filter_sql.push_str(&format!(" AND e.country = ?{}", *param_idx));
        params.push(Box::new(country.clone()));
        *param_idx += 1;
    }
    if let Some(ref page) = filter.filter_page {
        filter_sql.push_str(&format!(" AND position(?{} in e.url) > 0", *param_idx));
        params.push(Box::new(page.clone()));
        *param_idx += 1;
    }
    if let Some(ref referrer) = filter.filter_referrer {
        filter_sql.push_str(&format!(" AND e.referrer_domain = ?{}", *param_idx));
        params.push(Box::new(referrer.clone()));
        *param_idx += 1;
    }
    if let Some(ref browser) = filter.filter_browser {
        filter_sql.push_str(&format!(" AND e.browser = ?{}", *param_idx));
        params.push(Box::new(browser.clone()));
        *param_idx += 1;
    }
    if let Some(ref os) = filter.filter_os {
        filter_sql.push_str(&format!(" AND e.os = ?{}", *param_idx));
        params.push(Box::new(os.clone()));
        *param_idx += 1;
    }
    if let Some(ref device) = filter.filter_device {
        filter_sql.push_str(&format!(" AND e.device_type = ?{}", *param_idx));
        params.push(Box::new(device.clone()));
        *param_idx += 1;
    }
    if let Some(ref language) = filter.filter_language {
        filter_sql.push_str(&format!(" AND e.language = ?{}", *param_idx));
        params.push(Box::new(language.clone()));
        *param_idx += 1;
    }
    if let Some(ref utm_source) = filter.filter_utm_source {
        filter_sql.push_str(&format!(" AND e.utm_source = ?{}", *param_idx));
        params.push(Box::new(utm_source.clone()));
        *param_idx += 1;
    }
    if let Some(ref utm_medium) = filter.filter_utm_medium {
        filter_sql.push_str(&format!(" AND e.utm_medium = ?{}", *param_idx));
        params.push(Box::new(utm_medium.clone()));
        *param_idx += 1;
    }
    if let Some(ref utm_campaign) = filter.filter_utm_campaign {
        filter_sql.push_str(&format!(" AND e.utm_campaign = ?{}", *param_idx));
        params.push(Box::new(utm_campaign.clone()));
        *param_idx += 1;
    }
    if let Some(ref region) = filter.filter_region {
        filter_sql.push_str(&format!(" AND e.region = ?{}", *param_idx));
        params.push(Box::new(region.clone()));
        *param_idx += 1;
    }
    if let Some(ref city) = filter.filter_city {
        filter_sql.push_str(&format!(" AND e.city = ?{}", *param_idx));
        params.push(Box::new(city.clone()));
        *param_idx += 1;
    }
    if let Some(ref hostname) = filter.filter_hostname {
        filter_sql.push_str(&format!(
            " AND lower(regexp_extract(e.url, '^https?://([^/:?#]+)', 1)) = lower(?{})",
            *param_idx
        ));
        params.push(Box::new(hostname.clone()));
        *param_idx += 1;
    }
}

fn step_condition_sql(step: &FunnelStep, param_idx: usize) -> (String, String) {
    match (&step.step_type, &step.match_operator) {
        (StepType::PageView, MatchOperator::Equals) => (
            format!("e.event_type = 'pageview' AND e.url = ?{}", param_idx),
            step.match_value.clone(),
        ),
        (StepType::PageView, MatchOperator::Contains) => (
            format!(
                "e.event_type = 'pageview' AND position(?{} in e.url) > 0",
                param_idx
            ),
            step.match_value.clone(),
        ),
        (StepType::Event, MatchOperator::Equals) => (
            format!("e.event_type = 'event' AND e.event_name = ?{}", param_idx),
            step.match_value.clone(),
        ),
        (StepType::Event, MatchOperator::Contains) => (
            format!(
                "e.event_type = 'event' AND position(?{} in e.event_name) > 0",
                param_idx
            ),
            step.match_value.clone(),
        ),
    }
}

fn resolve_timezone(
    conn: &duckdb::Connection,
    website_id: &str,
    requested_timezone: Option<&str>,
) -> Result<Tz> {
    if let Some(raw) = requested_timezone {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("invalid_timezone"));
        }
        return trimmed
            .parse::<Tz>()
            .map_err(|_| anyhow!("invalid_timezone"));
    }

    let website_tz: String = conn
        .prepare("SELECT timezone FROM websites WHERE id = ?1")?
        .query_row(duckdb::params![website_id], |row| row.get(0))
        .unwrap_or_else(|_| "UTC".to_string());

    website_tz
        .parse::<Tz>()
        .or_else(|_| "UTC".parse::<Tz>())
        .map_err(|_| anyhow!("invalid_timezone"))
}

fn local_midnight_utc(tz: Tz, date: chrono::NaiveDate) -> Result<chrono::NaiveDateTime> {
    let naive = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid_date_boundary"))?;
    let zoned = match tz.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(a, b) => a.min(b),
        LocalResult::None => return Err(anyhow!("invalid_timezone_transition")),
    };
    Ok(zoned.with_timezone(&chrono::Utc).naive_utc())
}

fn utc_bounds_for_filter(
    tz: Tz,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
) -> Result<(String, String)> {
    let start_utc = local_midnight_utc(tz, start_date)?;
    let end_next_utc = local_midnight_utc(tz, end_date + chrono::Duration::days(1))?;
    Ok((
        start_utc.format("%Y-%m-%d %H:%M:%S").to_string(),
        end_next_utc.format("%Y-%m-%d %H:%M:%S").to_string(),
    ))
}

pub(crate) fn build_funnel_query(
    steps: &[FunnelStep],
    filter_sql: &str,
    first_match_param_idx: usize,
) -> (String, usize) {
    let mut ctes = Vec::new();
    ctes.push(format!(
        r#"
        scoped_events AS (
            SELECT
                e.session_id,
                e.created_at,
                e.event_type,
                e.url,
                e.event_name
            FROM events e
            WHERE e.website_id = ?1
              AND e.created_at >= ?2
              AND e.created_at < ?3
              {filter_sql}
        )
        "#
    ));

    let mut param_idx = first_match_param_idx;
    for (idx, step) in steps.iter().enumerate() {
        let step_num = idx + 1;
        let prev_step_num = idx;
        let (condition, _) = step_condition_sql(step, param_idx);
        param_idx += 1;

        let cte = if step_num == 1 {
            format!(
                r#"
                step_{step_num} AS (
                    SELECT e.session_id, MIN(e.created_at) AS matched_at
                    FROM scoped_events e
                    WHERE {condition}
                    GROUP BY e.session_id
                )
                "#
            )
        } else {
            format!(
                r#"
                step_{step_num} AS (
                    SELECT e.session_id, MIN(e.created_at) AS matched_at
                    FROM scoped_events e
                    JOIN step_{prev_step_num} prev ON prev.session_id = e.session_id
                    WHERE e.created_at > prev.matched_at
                      AND {condition}
                    GROUP BY e.session_id
                )
                "#
            )
        };
        ctes.push(cte);
    }

    let count_columns: Vec<String> = (1..=steps.len())
        .map(|i| format!("(SELECT COUNT(*) FROM step_{i}) AS step_{i}_count"))
        .collect();
    let sql = format!(
        "WITH {}\nSELECT {}",
        ctes.join(","),
        count_columns.join(", ")
    );
    (sql, param_idx)
}

pub(crate) fn compute_funnel_results(funnel: &Funnel, step_counts: &[i64]) -> FunnelResults {
    let total_entered = *step_counts.first().unwrap_or(&0);
    let mut steps = Vec::with_capacity(funnel.steps.len());

    for (idx, step) in funnel.steps.iter().enumerate() {
        let sessions_reached = *step_counts.get(idx).unwrap_or(&0);
        let drop_off_count = if idx + 1 < step_counts.len() {
            (sessions_reached - step_counts[idx + 1]).max(0)
        } else {
            0
        };
        let drop_off_rate = if idx + 1 < step_counts.len() && sessions_reached > 0 {
            drop_off_count as f64 / sessions_reached as f64
        } else {
            0.0
        };
        let conversion_rate_from_start = if total_entered > 0 {
            sessions_reached as f64 / total_entered as f64
        } else {
            0.0
        };
        let conversion_rate_from_previous = if idx == 0 {
            if sessions_reached > 0 {
                1.0
            } else {
                0.0
            }
        } else {
            let previous = step_counts[idx - 1];
            if previous > 0 {
                sessions_reached as f64 / previous as f64
            } else {
                0.0
            }
        };

        steps.push(FunnelStepResult {
            step_order: step.step_order,
            label: step.label.clone(),
            sessions_reached,
            drop_off_count,
            drop_off_rate,
            conversion_rate_from_start,
            conversion_rate_from_previous,
        });
    }

    let final_sessions = *step_counts.last().unwrap_or(&0);
    let final_conversion_rate = if total_entered > 0 {
        final_sessions as f64 / total_entered as f64
    } else {
        0.0
    };

    FunnelResults {
        funnel_id: funnel.id.clone(),
        name: funnel.name.clone(),
        total_sessions_entered: total_entered,
        final_conversion_rate,
        steps,
    }
}

pub async fn get_funnel_results_inner(
    db: &DuckDbBackend,
    website_id: &str,
    funnel_id: &str,
    filter: &AnalyticsFilter,
) -> Result<FunnelResults> {
    let funnel = get_funnel_inner(db, website_id, funnel_id)
        .await?
        .ok_or_else(|| anyhow!("Funnel not found"))?;
    if funnel.steps.is_empty() {
        return Err(anyhow!("funnel has no steps"));
    }

    let conn = db.conn.lock().await;
    let tz = resolve_timezone(&conn, website_id, filter.timezone.as_deref())?;
    let (start_str, end_str) = utc_bounds_for_filter(tz, filter.start_date, filter.end_date)?;

    let mut params: Vec<Box<dyn duckdb::types::ToSql>> = vec![
        Box::new(website_id.to_string()),
        Box::new(start_str),
        Box::new(end_str),
    ];
    let mut param_idx = 4;
    let mut filter_sql = String::new();
    append_event_filters(filter, &mut filter_sql, &mut params, &mut param_idx);

    let (sql, _next_param_idx) = build_funnel_query(&funnel.steps, &filter_sql, param_idx);
    for step in &funnel.steps {
        let (_, value) = step_condition_sql(step, param_idx);
        params.push(Box::new(value));
        param_idx += 1;
    }

    let step_count_len = funnel.steps.len();
    let param_refs: Vec<&dyn duckdb::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    if let Err(error) = conn.execute_batch("SET statement_timeout = '5000ms'") {
        tracing::warn!(%error, "Could not set DuckDB statement_timeout");
    }

    let step_counts_res: Result<Vec<i64>> = (|| {
        conn.prepare(&sql)?
            .query_row(param_refs.as_slice(), |row| {
                let mut counts = Vec::with_capacity(step_count_len);
                for idx in 0..step_count_len {
                    counts.push(row.get::<usize, i64>(idx)?);
                }
                Ok(counts)
            })
            .map_err(Into::into)
    })();

    if let Err(error) = conn.execute_batch("RESET statement_timeout") {
        tracing::warn!(%error, "Could not reset DuckDB statement_timeout");
    }

    let step_counts = step_counts_res?;

    Ok(compute_funnel_results(&funnel, &step_counts))
}

#[cfg(test)]
mod tests {
    use sparklytics_core::analytics::{Funnel, FunnelStep, MatchOperator, StepType};

    use super::{build_funnel_query, compute_funnel_results};

    fn sample_funnel() -> Funnel {
        Funnel {
            id: "fun_1".to_string(),
            website_id: "site_1".to_string(),
            name: "Signup".to_string(),
            steps: vec![
                FunnelStep {
                    id: "fstep_1".to_string(),
                    funnel_id: "fun_1".to_string(),
                    step_order: 1,
                    step_type: StepType::PageView,
                    match_value: "/pricing".to_string(),
                    match_operator: MatchOperator::Equals,
                    label: "Pricing".to_string(),
                    created_at: "2026-01-01 00:00:00".to_string(),
                },
                FunnelStep {
                    id: "fstep_2".to_string(),
                    funnel_id: "fun_1".to_string(),
                    step_order: 2,
                    step_type: StepType::Event,
                    match_value: "signup_completed".to_string(),
                    match_operator: MatchOperator::Equals,
                    label: "Signup".to_string(),
                    created_at: "2026-01-01 00:00:00".to_string(),
                },
            ],
            created_at: "2026-01-01 00:00:00".to_string(),
            updated_at: "2026-01-01 00:00:00".to_string(),
        }
    }

    #[test]
    fn build_funnel_query_contains_step_ctes() {
        let funnel = sample_funnel();
        let (sql, next_idx) = build_funnel_query(&funnel.steps, "", 4);
        assert!(sql.contains("step_1 AS"));
        assert!(sql.contains("step_2 AS"));
        assert!(sql.contains("(SELECT COUNT(*) FROM step_2)"));
        assert_eq!(next_idx, 6);
    }

    #[test]
    fn compute_funnel_results_dropoff_and_rates() {
        let funnel = sample_funnel();
        let results = compute_funnel_results(&funnel, &[10, 4]);
        assert_eq!(results.total_sessions_entered, 10);
        assert!((results.final_conversion_rate - 0.4).abs() < 0.0001);
        assert_eq!(results.steps[0].sessions_reached, 10);
        assert_eq!(results.steps[0].drop_off_count, 6);
        assert!((results.steps[1].conversion_rate_from_previous - 0.4).abs() < 0.0001);
        assert_eq!(results.steps[1].drop_off_count, 0);
    }
}
