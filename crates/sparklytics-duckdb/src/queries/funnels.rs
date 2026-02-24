use anyhow::{anyhow, Result};
use duckdb::Error;
use rand::Rng;

use sparklytics_core::analytics::{
    CreateFunnelRequest, CreateFunnelStepRequest, Funnel, FunnelStep, FunnelSummary, MatchOperator,
    StepType, UpdateFunnelRequest,
};

use crate::DuckDbBackend;

const MAX_FUNNELS_PER_WEBSITE: i64 = 20;

fn generate_funnel_id() -> String {
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
    format!("fun_{}", chars)
}

fn generate_funnel_step_id() -> String {
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
    format!("fstep_{}", chars)
}

fn step_type_to_str(step_type: &StepType) -> &'static str {
    match step_type {
        StepType::PageView => "page_view",
        StepType::Event => "event",
    }
}

fn step_type_from_str(raw: &str) -> Result<StepType> {
    match raw {
        "page_view" => Ok(StepType::PageView),
        "event" => Ok(StepType::Event),
        _ => Err(anyhow!("invalid step_type")),
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

fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(anyhow!("validation_error:name"));
    }
    if name.len() > 100 {
        return Err(anyhow!("validation_error:name"));
    }
    Ok(())
}

fn validate_steps(steps: &[CreateFunnelStepRequest]) -> Result<()> {
    if !(2..=8).contains(&steps.len()) {
        return Err(anyhow!("validation_error:steps"));
    }

    for step in steps {
        if step.match_value.trim().is_empty() {
            return Err(anyhow!("validation_error:match_value"));
        }
        if step.match_value.len() > 500 {
            return Err(anyhow!("validation_error:match_value"));
        }
        if let Some(label) = &step.label {
            if label.trim().is_empty() {
                return Err(anyhow!("validation_error:label"));
            }
            if label.len() > 120 {
                return Err(anyhow!("validation_error:label"));
            }
        }
    }
    Ok(())
}

fn is_duplicate_name_constraint(error: &Error) -> bool {
    let message = error.to_string();
    message.contains("UNIQUE constraint failed")
        && (message.contains("funnels.website_id") || message.contains("idx_funnels_website_name"))
}

fn load_funnel_steps(conn: &duckdb::Connection, funnel_id: &str) -> Result<Vec<FunnelStep>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT
            id,
            funnel_id,
            step_order,
            step_type,
            match_value,
            match_operator,
            label,
            CAST(created_at AS VARCHAR)
        FROM funnel_steps
        WHERE funnel_id = ?1
        ORDER BY step_order ASC
        "#,
    )?;

    let rows = stmt.query_map(duckdb::params![funnel_id], |row| {
        let step_type_raw: String = row.get(3)?;
        let match_op_raw: String = row.get(5)?;
        let step_order: i64 = row.get(2)?;
        Ok(FunnelStep {
            id: row.get(0)?,
            funnel_id: row.get(1)?,
            step_order: step_order as u32,
            step_type: step_type_from_str(&step_type_raw)
                .map_err(|_| duckdb::Error::InvalidQuery)?,
            match_value: row.get(4)?,
            match_operator: match_op_from_str(&match_op_raw)
                .map_err(|_| duckdb::Error::InvalidQuery)?,
            label: row.get(6)?,
            created_at: row.get(7)?,
        })
    })?;

    let mut steps = Vec::new();
    for row in rows {
        steps.push(row?);
    }
    Ok(steps)
}

fn get_funnel_with_conn(
    conn: &duckdb::Connection,
    website_id: &str,
    funnel_id: &str,
) -> Result<Option<Funnel>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT
            id,
            website_id,
            name,
            CAST(created_at AS VARCHAR),
            CAST(updated_at AS VARCHAR)
        FROM funnels
        WHERE website_id = ?1 AND id = ?2
        "#,
    )?;

    let row = match stmt.query_row(duckdb::params![website_id, funnel_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    }) {
        Ok(row) => Some(row),
        Err(Error::QueryReturnedNoRows) => None,
        Err(error) => return Err(error.into()),
    };

    let Some((id, website_id, name, created_at, updated_at)) = row else {
        return Ok(None);
    };

    let steps = load_funnel_steps(conn, &id)?;
    Ok(Some(Funnel {
        id,
        website_id,
        name,
        steps,
        created_at,
        updated_at,
    }))
}

pub async fn list_funnels_inner(
    db: &DuckDbBackend,
    website_id: &str,
) -> Result<Vec<FunnelSummary>> {
    let conn = db.conn.lock().await;
    let mut stmt = conn.prepare(
        r#"
        SELECT
            f.id,
            f.website_id,
            f.name,
            CAST(COUNT(fs.id) AS BIGINT) AS step_count,
            CAST(f.created_at AS VARCHAR),
            CAST(f.updated_at AS VARCHAR)
        FROM funnels f
        LEFT JOIN funnel_steps fs ON fs.funnel_id = f.id
        WHERE f.website_id = ?1
        GROUP BY f.id, f.website_id, f.name, f.created_at, f.updated_at
        ORDER BY f.created_at DESC, f.id DESC
        "#,
    )?;

    let rows = stmt.query_map(duckdb::params![website_id], |row| {
        let step_count: i64 = row.get(3)?;
        Ok(FunnelSummary {
            id: row.get(0)?,
            website_id: row.get(1)?,
            name: row.get(2)?,
            step_count: step_count as u32,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?;

    let mut funnels = Vec::new();
    for row in rows {
        funnels.push(row?);
    }
    Ok(funnels)
}

pub async fn get_funnel_inner(
    db: &DuckDbBackend,
    website_id: &str,
    funnel_id: &str,
) -> Result<Option<Funnel>> {
    let conn = db.conn.lock().await;
    get_funnel_with_conn(&conn, website_id, funnel_id)
}

pub async fn create_funnel_inner(
    db: &DuckDbBackend,
    website_id: &str,
    req: CreateFunnelRequest,
) -> Result<Funnel> {
    validate_name(&req.name)?;
    validate_steps(&req.steps)?;

    let mut conn = db.conn.lock().await;

    let funnel_id = generate_funnel_id();
    let tx = conn.transaction()?;
    let count: i64 = tx
        .prepare("SELECT COUNT(*) FROM funnels WHERE website_id = ?1")?
        .query_row(duckdb::params![website_id], |row| row.get(0))?;
    if count >= MAX_FUNNELS_PER_WEBSITE {
        return Err(anyhow!("limit_exceeded"));
    }

    let duplicate_count: i64 = tx
        .prepare("SELECT COUNT(*) FROM funnels WHERE website_id = ?1 AND name = ?2")?
        .query_row(duckdb::params![website_id, &req.name], |row| row.get(0))?;
    if duplicate_count > 0 {
        return Err(anyhow!("duplicate_name"));
    }

    if let Err(error) = tx.execute(
        r#"
        INSERT INTO funnels (
            id,
            website_id,
            name,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
        duckdb::params![&funnel_id, website_id, &req.name],
    ) {
        if is_duplicate_name_constraint(&error) {
            return Err(anyhow!("duplicate_name"));
        }
        return Err(error.into());
    }

    for (idx, step) in req.steps.iter().enumerate() {
        let step_id = generate_funnel_step_id();
        let match_operator = step.match_operator.clone().unwrap_or_default();
        let label = step
            .label
            .clone()
            .unwrap_or_else(|| step.match_value.clone());

        tx.execute(
            r#"
            INSERT INTO funnel_steps (
                id,
                funnel_id,
                step_order,
                step_type,
                match_value,
                match_operator,
                label,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP)
            "#,
            duckdb::params![
                step_id,
                &funnel_id,
                (idx + 1) as i64,
                step_type_to_str(&step.step_type),
                &step.match_value,
                match_op_to_str(&match_operator),
                label,
            ],
        )?;
    }

    tx.commit()?;

    get_funnel_with_conn(&conn, website_id, &funnel_id)?
        .ok_or_else(|| anyhow!("failed to load created funnel"))
}

pub async fn update_funnel_inner(
    db: &DuckDbBackend,
    website_id: &str,
    funnel_id: &str,
    req: UpdateFunnelRequest,
) -> Result<Option<Funnel>> {
    if req.name.is_none() && req.steps.is_none() {
        return get_funnel_inner(db, website_id, funnel_id).await;
    }

    if let Some(name) = &req.name {
        validate_name(name)?;
    }
    if let Some(steps) = &req.steps {
        validate_steps(steps)?;
    }

    let mut conn = db.conn.lock().await;
    let exists: i64 = conn
        .prepare("SELECT COUNT(*) FROM funnels WHERE website_id = ?1 AND id = ?2")?
        .query_row(duckdb::params![website_id, funnel_id], |row| row.get(0))?;
    if exists == 0 {
        return Ok(None);
    }

    if let Some(name) = &req.name {
        let duplicate_count: i64 = conn
            .prepare(
                "SELECT COUNT(*) FROM funnels WHERE website_id = ?1 AND name = ?2 AND id != ?3",
            )?
            .query_row(duckdb::params![website_id, name, funnel_id], |row| {
                row.get(0)
            })?;
        if duplicate_count > 0 {
            return Err(anyhow!("duplicate_name"));
        }
    }

    let tx = conn.transaction()?;

    if let Some(name) = &req.name {
        if let Err(error) = tx.execute(
            "UPDATE funnels SET name = ?1, updated_at = CURRENT_TIMESTAMP WHERE website_id = ?2 AND id = ?3",
            duckdb::params![name, website_id, funnel_id],
        ) {
            if is_duplicate_name_constraint(&error) {
                return Err(anyhow!("duplicate_name"));
            }
            return Err(error.into());
        }
    } else {
        tx.execute(
            "UPDATE funnels SET updated_at = CURRENT_TIMESTAMP WHERE website_id = ?1 AND id = ?2",
            duckdb::params![website_id, funnel_id],
        )?;
    }

    if let Some(steps) = &req.steps {
        tx.execute(
            "DELETE FROM funnel_steps WHERE funnel_id = ?1",
            duckdb::params![funnel_id],
        )?;

        for (idx, step) in steps.iter().enumerate() {
            let step_id = generate_funnel_step_id();
            let match_operator = step.match_operator.clone().unwrap_or_default();
            let label = step
                .label
                .clone()
                .unwrap_or_else(|| step.match_value.clone());
            tx.execute(
                r#"
                INSERT INTO funnel_steps (
                    id,
                    funnel_id,
                    step_order,
                    step_type,
                    match_value,
                    match_operator,
                    label,
                    created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP)
                "#,
                duckdb::params![
                    step_id,
                    funnel_id,
                    (idx + 1) as i64,
                    step_type_to_str(&step.step_type),
                    &step.match_value,
                    match_op_to_str(&match_operator),
                    label,
                ],
            )?;
        }
    }

    tx.commit()?;
    get_funnel_with_conn(&conn, website_id, funnel_id)
}

pub async fn delete_funnel_inner(
    db: &DuckDbBackend,
    website_id: &str,
    funnel_id: &str,
) -> Result<bool> {
    let mut conn = db.conn.lock().await;
    let tx = conn.transaction()?;

    let exists: i64 = tx
        .prepare("SELECT COUNT(*) FROM funnels WHERE website_id = ?1 AND id = ?2")?
        .query_row(duckdb::params![website_id, funnel_id], |row| row.get(0))?;
    if exists == 0 {
        return Ok(false);
    }

    tx.execute(
        "DELETE FROM funnel_steps WHERE funnel_id = ?1",
        duckdb::params![funnel_id],
    )?;
    tx.execute(
        "DELETE FROM funnels WHERE website_id = ?1 AND id = ?2",
        duckdb::params![website_id, funnel_id],
    )?;
    tx.commit()?;
    Ok(true)
}
