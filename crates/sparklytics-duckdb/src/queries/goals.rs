use anyhow::{anyhow, Result};
use chrono::Duration;
use rand::Rng;

use sparklytics_core::analytics::{
    AnalyticsFilter, CreateGoalRequest, Goal, GoalStats, GoalType, MatchOperator, UpdateGoalRequest,
};

use crate::DuckDbBackend;

const MAX_GOALS_PER_WEBSITE: i64 = 50;

fn generate_goal_id() -> String {
    let mut rng = rand::thread_rng();
    let chars: String = (0..21)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect();
    format!("goal_{}", chars)
}

fn goal_type_to_str(goal_type: &GoalType) -> &'static str {
    match goal_type {
        GoalType::PageView => "page_view",
        GoalType::Event => "event",
    }
}

fn goal_type_from_str(raw: &str) -> Result<GoalType> {
    match raw {
        "page_view" => Ok(GoalType::PageView),
        "event" => Ok(GoalType::Event),
        _ => Err(anyhow!("invalid goal_type")),
    }
}

fn match_op_to_str(op: &MatchOperator) -> &'static str {
    match op {
        MatchOperator::Equals => "equals",
        MatchOperator::Contains => "contains",
    }
}

fn match_op_from_str(raw: &str) -> Result<MatchOperator> {
    match raw {
        "equals" => Ok(MatchOperator::Equals),
        "contains" => Ok(MatchOperator::Contains),
        _ => Err(anyhow!("invalid match_operator")),
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

fn map_goal_row(row: &duckdb::Row<'_>) -> Result<Goal, duckdb::Error> {
    let goal_type_raw: String = row.get(3)?;
    let match_op_raw: String = row.get(5)?;
    Ok(Goal {
        id: row.get(0)?,
        website_id: row.get(1)?,
        name: row.get(2)?,
        goal_type: goal_type_from_str(&goal_type_raw).map_err(|_| duckdb::Error::InvalidQuery)?,
        match_value: row.get(4)?,
        match_operator: match_op_from_str(&match_op_raw)
            .map_err(|_| duckdb::Error::InvalidQuery)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn query_period_stats(
    conn: &duckdb::Connection,
    website_id: &str,
    goal: &Goal,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
    filter: &AnalyticsFilter,
) -> Result<(i64, i64, i64, bool)> {
    let start_str = start_date.format("%Y-%m-%d").to_string();
    let end_str = (end_date + Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    let mut params: Vec<Box<dyn duckdb::types::ToSql>> = vec![
        Box::new(website_id.to_string()),
        Box::new(start_str),
        Box::new(end_str),
    ];
    let mut param_idx = 4;
    let mut filter_sql = String::new();
    append_event_filters(filter, &mut filter_sql, &mut params, &mut param_idx);

    let match_sql = match (&goal.goal_type, &goal.match_operator) {
        (GoalType::PageView, MatchOperator::Equals) => {
            params.push(Box::new(goal.match_value.clone()));
            format!("e.event_type = 'pageview' AND e.url = ?{}", param_idx)
        }
        (GoalType::PageView, MatchOperator::Contains) => {
            params.push(Box::new(format!("%{}%", goal.match_value)));
            format!("e.event_type = 'pageview' AND e.url LIKE ?{}", param_idx)
        }
        (GoalType::Event, MatchOperator::Equals) => {
            params.push(Box::new(goal.match_value.clone()));
            format!("e.event_type = 'event' AND e.event_name = ?{}", param_idx)
        }
        (GoalType::Event, MatchOperator::Contains) => {
            params.push(Box::new(format!("%{}%", goal.match_value)));
            format!(
                "e.event_type = 'event' AND e.event_name LIKE ?{}",
                param_idx
            )
        }
    };

    let sql = format!(
        r#"
        WITH scoped_events AS (
            SELECT e.session_id, e.url, e.event_name, e.event_type
            FROM events e
            WHERE e.website_id = ?1
              AND e.created_at >= ?2
              AND e.created_at < ?3
              {filter_sql}
        ),
        goal_matches AS (
            SELECT e.session_id
            FROM scoped_events e
            WHERE {match_sql}
        ),
        session_totals AS (
            SELECT COUNT(DISTINCT session_id) AS total_sessions
            FROM scoped_events
        )
        SELECT
            (SELECT COUNT(*) FROM goal_matches) AS conversions,
            (SELECT COUNT(DISTINCT session_id) FROM goal_matches) AS converting_sessions,
            (SELECT total_sessions FROM session_totals) AS total_sessions,
            EXISTS(SELECT 1 FROM scoped_events LIMIT 1) AS has_period_data
        "#
    );

    let param_refs: Vec<&dyn duckdb::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let row = conn
        .prepare(&sql)?
        .query_row(param_refs.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, bool>(3)?,
            ))
        })?;
    Ok(row)
}

pub async fn list_goals_inner(db: &DuckDbBackend, website_id: &str) -> Result<Vec<Goal>> {
    let conn = db.conn.lock().await;
    let mut stmt = conn.prepare(
        r#"
        SELECT
            id,
            website_id,
            name,
            goal_type,
            match_value,
            match_operator,
            CAST(created_at AS VARCHAR),
            CAST(updated_at AS VARCHAR)
        FROM goals
        WHERE website_id = ?1
        ORDER BY created_at DESC, id DESC
        "#,
    )?;
    let rows = stmt.query_map(duckdb::params![website_id], map_goal_row)?;

    let mut goals = Vec::new();
    for row in rows {
        goals.push(row?);
    }
    Ok(goals)
}

pub async fn count_goals_inner(db: &DuckDbBackend, website_id: &str) -> Result<i64> {
    let conn = db.conn.lock().await;
    let count = conn
        .prepare("SELECT COUNT(*) FROM goals WHERE website_id = ?1")?
        .query_row(duckdb::params![website_id], |row| row.get(0))?;
    Ok(count)
}

pub async fn goal_name_exists_inner(
    db: &DuckDbBackend,
    website_id: &str,
    name: &str,
    exclude_goal_id: Option<&str>,
) -> Result<bool> {
    let conn = db.conn.lock().await;
    let exists: i64 = if let Some(exclude_id) = exclude_goal_id {
        conn.prepare("SELECT COUNT(*) FROM goals WHERE website_id = ?1 AND name = ?2 AND id != ?3")?
            .query_row(duckdb::params![website_id, name, exclude_id], |row| {
                row.get(0)
            })?
    } else {
        conn.prepare("SELECT COUNT(*) FROM goals WHERE website_id = ?1 AND name = ?2")?
            .query_row(duckdb::params![website_id, name], |row| row.get(0))?
    };
    Ok(exists > 0)
}

pub async fn create_goal_inner(
    db: &DuckDbBackend,
    website_id: &str,
    req: CreateGoalRequest,
) -> Result<Goal> {
    let conn = db.conn.lock().await;

    let goals_count: i64 = conn
        .prepare("SELECT COUNT(*) FROM goals WHERE website_id = ?1")?
        .query_row(duckdb::params![website_id], |row| row.get(0))?;
    if goals_count >= MAX_GOALS_PER_WEBSITE {
        return Err(anyhow!("limit_exceeded"));
    }

    let duplicate_name_count: i64 = conn
        .prepare("SELECT COUNT(*) FROM goals WHERE website_id = ?1 AND name = ?2")?
        .query_row(duckdb::params![website_id, &req.name], |row| row.get(0))?;
    if duplicate_name_count > 0 {
        return Err(anyhow!("duplicate_name"));
    }

    let id = generate_goal_id();
    let match_operator = req.match_operator.unwrap_or_default();

    conn.execute(
        r#"
        INSERT INTO goals (
            id,
            website_id,
            name,
            goal_type,
            match_value,
            match_operator,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
        duckdb::params![
            id,
            website_id,
            req.name,
            goal_type_to_str(&req.goal_type),
            req.match_value,
            match_op_to_str(&match_operator),
        ],
    )?;

    let mut stmt = conn.prepare(
        r#"
        SELECT
            id,
            website_id,
            name,
            goal_type,
            match_value,
            match_operator,
            CAST(created_at AS VARCHAR),
            CAST(updated_at AS VARCHAR)
        FROM goals
        WHERE id = ?1
        "#,
    )?;
    let goal = stmt.query_row(duckdb::params![id], map_goal_row)?;
    Ok(goal)
}

pub async fn update_goal_inner(
    db: &DuckDbBackend,
    website_id: &str,
    goal_id: &str,
    req: UpdateGoalRequest,
) -> Result<Goal> {
    let conn = db.conn.lock().await;
    let mut stmt = conn.prepare(
        r#"
        SELECT
            id,
            website_id,
            name,
            goal_type,
            match_value,
            match_operator,
            CAST(created_at AS VARCHAR),
            CAST(updated_at AS VARCHAR)
        FROM goals
        WHERE website_id = ?1 AND id = ?2
        "#,
    )?;
    let existing = stmt
        .query_row(duckdb::params![website_id, goal_id], map_goal_row)
        .ok();
    let existing = existing.ok_or_else(|| anyhow!("Goal not found"))?;

    let original_name = existing.name.clone();
    let next_name = req.name.unwrap_or(existing.name);
    let next_match_value = req.match_value.unwrap_or(existing.match_value);
    let next_match_operator = req.match_operator.unwrap_or(existing.match_operator);

    if next_name != original_name {
        let duplicate_name_count: i64 = conn
            .prepare("SELECT COUNT(*) FROM goals WHERE website_id = ?1 AND name = ?2 AND id != ?3")?
            .query_row(duckdb::params![website_id, &next_name, goal_id], |row| {
                row.get(0)
            })?;
        if duplicate_name_count > 0 {
            return Err(anyhow!("duplicate_name"));
        }
    }

    conn.execute(
        r#"
        UPDATE goals
        SET
            name = ?1,
            match_value = ?2,
            match_operator = ?3,
            updated_at = CURRENT_TIMESTAMP
        WHERE website_id = ?4 AND id = ?5
        "#,
        duckdb::params![
            next_name,
            next_match_value,
            match_op_to_str(&next_match_operator),
            website_id,
            goal_id
        ],
    )?;

    let mut updated_stmt = conn.prepare(
        r#"
        SELECT
            id,
            website_id,
            name,
            goal_type,
            match_value,
            match_operator,
            CAST(created_at AS VARCHAR),
            CAST(updated_at AS VARCHAR)
        FROM goals
        WHERE website_id = ?1 AND id = ?2
        "#,
    )?;
    let goal = updated_stmt.query_row(duckdb::params![website_id, goal_id], map_goal_row)?;
    Ok(goal)
}

pub async fn delete_goal_inner(db: &DuckDbBackend, website_id: &str, goal_id: &str) -> Result<()> {
    let conn = db.conn.lock().await;
    conn.execute(
        "DELETE FROM goals WHERE website_id = ?1 AND id = ?2",
        duckdb::params![website_id, goal_id],
    )?;
    Ok(())
}

pub async fn get_goal_stats_inner(
    db: &DuckDbBackend,
    website_id: &str,
    goal_id: &str,
    filter: &AnalyticsFilter,
) -> Result<GoalStats> {
    let conn = db.conn.lock().await;

    let mut stmt = conn.prepare(
        r#"
        SELECT
            id,
            website_id,
            name,
            goal_type,
            match_value,
            match_operator,
            CAST(created_at AS VARCHAR),
            CAST(updated_at AS VARCHAR)
        FROM goals
        WHERE website_id = ?1 AND id = ?2
        "#,
    )?;
    let goal = stmt
        .query_row(duckdb::params![website_id, goal_id], map_goal_row)
        .ok()
        .ok_or_else(|| anyhow!("Goal not found"))?;

    let (conversions, converting_sessions, total_sessions, _) = query_period_stats(
        &conn,
        website_id,
        &goal,
        filter.start_date,
        filter.end_date,
        filter,
    )?;

    let range_days = (filter.end_date - filter.start_date).num_days() + 1;
    let prev_end = filter.start_date - Duration::days(1);
    let prev_start = prev_end - Duration::days(range_days - 1);
    let (prev_conversions, prev_converting_sessions, prev_total_sessions, has_prev_period_data) =
        query_period_stats(&conn, website_id, &goal, prev_start, prev_end, filter)?;

    let conversion_rate = if total_sessions == 0 {
        0.0
    } else {
        converting_sessions as f64 / total_sessions as f64
    };
    let prev_conversion_rate = if prev_total_sessions == 0 {
        0.0
    } else {
        prev_converting_sessions as f64 / prev_total_sessions as f64
    };

    let trend_pct = if has_prev_period_data && prev_conversion_rate > 0.0 {
        Some(((conversion_rate - prev_conversion_rate) / prev_conversion_rate) * 100.0)
    } else {
        None
    };

    Ok(GoalStats {
        goal_id: goal.id,
        conversions,
        converting_sessions,
        total_sessions,
        conversion_rate,
        prev_conversions: if has_prev_period_data {
            Some(prev_conversions)
        } else {
            None
        },
        prev_conversion_rate: if has_prev_period_data {
            Some(prev_conversion_rate)
        } else {
            None
        },
        trend_pct,
    })
}
