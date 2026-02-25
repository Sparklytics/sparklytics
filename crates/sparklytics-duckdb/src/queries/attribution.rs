use std::collections::{BTreeMap, HashMap};

use anyhow::{anyhow, Result};

use sparklytics_core::analytics::{
    AnalyticsFilter, AttributionModel, AttributionQuery, AttributionResponse, AttributionRow,
    AttributionTotals, GoalType, GoalValueMode, MatchOperator, RevenueSummary,
};

use crate::DuckDbBackend;

#[derive(Debug, Clone)]
struct GoalDefinition {
    goal_type: GoalType,
    match_value: String,
    match_operator: MatchOperator,
    value_mode: GoalValueMode,
    fixed_value: Option<f64>,
    value_property_key: Option<String>,
}

#[derive(Debug, Clone)]
struct EventRow {
    session_id: String,
    event_type: String,
    url: String,
    event_name: Option<String>,
    event_data: Option<String>,
    utm_source: Option<String>,
    utm_medium: Option<String>,
    referrer_domain: Option<String>,
}

fn goal_type_from_str(raw: &str) -> Result<GoalType> {
    match raw {
        "page_view" => Ok(GoalType::PageView),
        "event" => Ok(GoalType::Event),
        _ => Err(anyhow!("invalid goal_type")),
    }
}

fn match_op_from_str(raw: &str) -> Result<MatchOperator> {
    match raw {
        "equals" => Ok(MatchOperator::Equals),
        "contains" => Ok(MatchOperator::Contains),
        _ => Err(anyhow!("invalid match_operator")),
    }
}

fn value_mode_from_str(raw: &str) -> Result<GoalValueMode> {
    match raw {
        "none" => Ok(GoalValueMode::None),
        "fixed" => Ok(GoalValueMode::Fixed),
        "event_property" => Ok(GoalValueMode::EventProperty),
        _ => Err(anyhow!("invalid value_mode")),
    }
}

fn append_event_filters(
    filter: &AnalyticsFilter,
    filter_sql: &mut String,
    params: &mut Vec<Box<dyn duckdb::types::ToSql>>,
    param_idx: &mut usize,
) {
    if let Some(ref country) = filter.filter_country {
        filter_sql.push_str(&format!(" AND e.country = ?{}", *param_idx));
        params.push(Box::new(country.clone()));
        *param_idx += 1;
    }
    if let Some(ref page) = filter.filter_page {
        filter_sql.push_str(&format!(" AND e.url LIKE ?{}", *param_idx));
        params.push(Box::new(format!("%{}%", page)));
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
            " AND lower(regexp_extract(e.url, '^https?://([^/?#]+)', 1)) = lower(?{})",
            *param_idx
        ));
        params.push(Box::new(hostname.clone()));
        *param_idx += 1;
    }
}

fn path_with_query(url: &str) -> String {
    if let Some(scheme_idx) = url.find("://") {
        let rest = &url[(scheme_idx + 3)..];
        if let Some(path_idx) = rest.find('/') {
            return rest[path_idx..].to_string();
        }
        return "/".to_string();
    }
    url.to_string()
}

fn is_goal_match(goal: &GoalDefinition, row: &EventRow) -> bool {
    match (&goal.goal_type, &goal.match_operator) {
        (GoalType::PageView, MatchOperator::Equals) => {
            row.event_type == "pageview" && path_with_query(&row.url) == goal.match_value
        }
        (GoalType::PageView, MatchOperator::Contains) => {
            row.event_type == "pageview" && row.url.contains(&goal.match_value)
        }
        (GoalType::Event, MatchOperator::Equals) => {
            row.event_type == "event"
                && row.event_name.as_deref() == Some(goal.match_value.as_str())
        }
        (GoalType::Event, MatchOperator::Contains) => {
            row.event_type == "event"
                && row
                    .event_name
                    .as_deref()
                    .map(|name| name.contains(&goal.match_value))
                    .unwrap_or(false)
        }
    }
}

fn channel_for_event(row: &EventRow) -> String {
    let source = row
        .utm_source
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .or_else(|| {
            row.referrer_domain
                .as_ref()
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
                .map(|v| v.to_string())
        })
        .unwrap_or_else(|| "(direct)".to_string());

    let medium = row
        .utm_medium
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .unwrap_or_else(|| {
            if row
                .utm_source
                .as_ref()
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false)
            {
                "utm".to_string()
            } else if row
                .referrer_domain
                .as_ref()
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false)
            {
                "referral".to_string()
            } else {
                "direct".to_string()
            }
        });

    format!("{source} / {medium}")
}

fn parse_revenue(goal: &GoalDefinition, event_data: Option<&str>) -> f64 {
    match goal.value_mode {
        GoalValueMode::None => 0.0,
        GoalValueMode::Fixed => goal.fixed_value.unwrap_or(0.0),
        GoalValueMode::EventProperty => {
            let Some(key) = goal.value_property_key.as_deref() else {
                return 0.0;
            };
            let Some(raw) = event_data else {
                return 0.0;
            };
            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw) else {
                return 0.0;
            };
            let Some(value) = parsed.get(key) else {
                return 0.0;
            };
            if let Some(number) = value.as_f64() {
                return number;
            }
            if let Some(text) = value.as_str() {
                return text.parse::<f64>().unwrap_or(0.0);
            }
            0.0
        }
    }
}

fn fetch_goal(
    conn: &duckdb::Connection,
    website_id: &str,
    goal_id: &str,
) -> Result<GoalDefinition> {
    let mut stmt = conn.prepare(
        r#"
        SELECT goal_type, match_value, match_operator, value_mode, fixed_value, value_property_key
        FROM goals
        WHERE website_id = ?1 AND id = ?2
        "#,
    )?;

    let row = stmt.query_row(duckdb::params![website_id, goal_id], |row| {
        let goal_type: String = row.get(0)?;
        let match_value: String = row.get(1)?;
        let match_operator: String = row.get(2)?;
        let value_mode: String = row.get(3)?;
        let fixed_value: Option<f64> = row.get(4)?;
        let value_property_key: Option<String> = row.get(5)?;
        Ok((
            goal_type,
            match_value,
            match_operator,
            value_mode,
            fixed_value,
            value_property_key,
        ))
    });

    let (goal_type, match_value, match_operator, value_mode, fixed_value, value_property_key) =
        row.map_err(|_| anyhow!("Goal not found"))?;

    Ok(GoalDefinition {
        goal_type: goal_type_from_str(&goal_type)?,
        match_value,
        match_operator: match_op_from_str(&match_operator)?,
        value_mode: value_mode_from_str(&value_mode)?,
        fixed_value,
        value_property_key,
    })
}

fn aggregate_rows(
    rows: Vec<EventRow>,
    goal: &GoalDefinition,
    model: &AttributionModel,
) -> (Vec<AttributionRow>, AttributionTotals) {
    let mut sessions: BTreeMap<String, Vec<EventRow>> = BTreeMap::new();
    for row in rows {
        sessions
            .entry(row.session_id.clone())
            .or_default()
            .push(row);
    }

    let mut by_channel: HashMap<String, (i64, f64)> = HashMap::new();
    let mut total_conversions: i64 = 0;
    let mut total_revenue: f64 = 0.0;

    for events in sessions.values() {
        let mut touch_channels: Vec<String> = Vec::with_capacity(events.len());

        for row in events {
            let channel = channel_for_event(row);
            touch_channels.push(channel.clone());

            if !is_goal_match(goal, row) {
                continue;
            }

            let attributed_channel = match model {
                AttributionModel::FirstTouch => {
                    touch_channels.first().cloned().unwrap_or(channel.clone())
                }
                AttributionModel::LastTouch => touch_channels.last().cloned().unwrap_or(channel),
            };

            let revenue = parse_revenue(goal, row.event_data.as_deref());
            let entry = by_channel.entry(attributed_channel).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += revenue;
            total_conversions += 1;
            total_revenue += revenue;
        }
    }

    let mut rows = by_channel
        .into_iter()
        .map(|(channel, (conversions, revenue))| AttributionRow {
            channel,
            conversions,
            revenue,
            share: if total_conversions == 0 {
                0.0
            } else {
                conversions as f64 / total_conversions as f64
            },
        })
        .collect::<Vec<_>>();

    rows.sort_by(|a, b| {
        b.conversions.cmp(&a.conversions).then_with(|| {
            b.revenue
                .partial_cmp(&a.revenue)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    (
        rows,
        AttributionTotals {
            conversions: total_conversions,
            revenue: total_revenue,
        },
    )
}

pub async fn get_attribution_inner(
    db: &DuckDbBackend,
    website_id: &str,
    filter: &AnalyticsFilter,
    query: &AttributionQuery,
) -> Result<AttributionResponse> {
    let conn = db.conn.lock().await;
    let goal = fetch_goal(&conn, website_id, &query.goal_id)?;

    let start_str = filter.start_date.format("%Y-%m-%d").to_string();
    let end_str = (filter.end_date + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    let mut params: Vec<Box<dyn duckdb::types::ToSql>> = vec![
        Box::new(website_id.to_string()),
        Box::new(start_str),
        Box::new(end_str),
    ];
    let mut filter_sql = String::new();
    let mut param_idx = 4;
    append_event_filters(filter, &mut filter_sql, &mut params, &mut param_idx);

    let sql = format!(
        r#"
        SELECT
            e.session_id,
            e.event_type,
            e.url,
            e.event_name,
            e.event_data,
            e.utm_source,
            e.utm_medium,
            e.referrer_domain,
            CAST(e.created_at AS VARCHAR)
        FROM events e
        WHERE e.website_id = ?1
          AND e.created_at >= ?2
          AND e.created_at < ?3
          {filter_sql}
        ORDER BY e.session_id ASC, e.created_at ASC, e.id ASC
        "#
    );

    let param_refs: Vec<&dyn duckdb::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(EventRow {
            session_id: row.get(0)?,
            event_type: row.get(1)?,
            url: row.get(2)?,
            event_name: row.get(3)?,
            event_data: row.get(4)?,
            utm_source: row.get(5)?,
            utm_medium: row.get(6)?,
            referrer_domain: row.get(7)?,
        })
    })?;

    let mut events = Vec::new();
    for row in rows {
        events.push(row?);
    }

    let (rows, totals) = aggregate_rows(events, &goal, &query.model);

    Ok(AttributionResponse {
        goal_id: query.goal_id.clone(),
        model: query.model.clone(),
        rows,
        totals,
    })
}

pub async fn get_revenue_summary_inner(
    db: &DuckDbBackend,
    website_id: &str,
    filter: &AnalyticsFilter,
    query: &AttributionQuery,
) -> Result<RevenueSummary> {
    let attribution = get_attribution_inner(db, website_id, filter, query).await?;
    Ok(RevenueSummary {
        goal_id: attribution.goal_id,
        model: attribution.model,
        conversions: attribution.totals.conversions,
        revenue: attribution.totals.revenue,
    })
}
