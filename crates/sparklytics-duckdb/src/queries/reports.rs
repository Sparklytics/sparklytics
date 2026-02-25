use anyhow::{anyhow, Result};
use rand::Rng;

use sparklytics_core::analytics::{
    CreateReportRequest, ReportConfig, ReportType, SavedReport, SavedReportSummary,
    UpdateReportRequest,
};

use crate::DuckDbBackend;

const MAX_REPORTS_PER_WEBSITE: i64 = 100;

fn generate_report_id() -> String {
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
    format!("report_{}", chars)
}

fn parse_config(config_json: &str) -> Result<ReportConfig> {
    serde_json::from_str(config_json).map_err(|e| anyhow!("invalid stored report config_json: {e}"))
}

fn report_type_from_str(raw: &str) -> Result<ReportType> {
    match raw {
        "stats" => Ok(ReportType::Stats),
        "pageviews" => Ok(ReportType::Pageviews),
        "metrics" => Ok(ReportType::Metrics),
        "events" => Ok(ReportType::Events),
        _ => Err(anyhow!("invalid report_type")),
    }
}

fn map_saved_report_row(row: &duckdb::Row<'_>) -> Result<SavedReport, duckdb::Error> {
    let config_json: String = row.get(4)?;
    let config = parse_config(&config_json).map_err(|_| duckdb::Error::InvalidQuery)?;
    Ok(SavedReport {
        id: row.get(0)?,
        website_id: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        config,
        last_run_at: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn get_report_by_id(
    conn: &duckdb::Connection,
    website_id: &str,
    report_id: &str,
) -> Result<Option<SavedReport>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT
            id,
            website_id,
            name,
            description,
            config_json,
            CAST(last_run_at AS VARCHAR),
            CAST(created_at AS VARCHAR),
            CAST(updated_at AS VARCHAR)
        FROM saved_reports
        WHERE website_id = ?1 AND id = ?2
        "#,
    )?;
    let report = stmt
        .query_row(duckdb::params![website_id, report_id], map_saved_report_row)
        .ok();
    Ok(report)
}

pub async fn list_reports_inner(
    db: &DuckDbBackend,
    website_id: &str,
) -> Result<Vec<SavedReportSummary>> {
    let conn = db.conn.lock().await;
    let mut stmt = conn.prepare(
        r#"
        SELECT
            id,
            name,
            description,
            COALESCE(JSON_EXTRACT_STRING(config_json, '$.report_type'), 'stats') AS report_type,
            CAST(last_run_at AS VARCHAR),
            CAST(created_at AS VARCHAR),
            CAST(updated_at AS VARCHAR)
        FROM saved_reports
        WHERE website_id = ?1
        ORDER BY lower(name) ASC, created_at ASC, id ASC
        "#,
    )?;

    let rows = stmt.query_map(duckdb::params![website_id], |row| {
        let report_type_raw: String = row.get(3)?;
        let report_type =
            report_type_from_str(&report_type_raw).map_err(|_| duckdb::Error::InvalidQuery)?;
        Ok(SavedReportSummary {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            report_type,
            last_run_at: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?;

    let mut reports = Vec::new();
    for row in rows {
        reports.push(row?);
    }
    Ok(reports)
}

pub async fn get_report_inner(
    db: &DuckDbBackend,
    website_id: &str,
    report_id: &str,
) -> Result<Option<SavedReport>> {
    let conn = db.conn.lock().await;
    get_report_by_id(&conn, website_id, report_id)
}

pub async fn count_reports_inner(db: &DuckDbBackend, website_id: &str) -> Result<i64> {
    let conn = db.conn.lock().await;
    let count = conn
        .prepare("SELECT COUNT(*) FROM saved_reports WHERE website_id = ?1")?
        .query_row(duckdb::params![website_id], |row| row.get(0))?;
    Ok(count)
}

pub async fn report_name_exists_inner(
    db: &DuckDbBackend,
    website_id: &str,
    name: &str,
    exclude_report_id: Option<&str>,
) -> Result<bool> {
    let conn = db.conn.lock().await;
    let exists: i64 = if let Some(exclude_id) = exclude_report_id {
        conn.prepare(
            "SELECT COUNT(*) FROM saved_reports WHERE website_id = ?1 AND name = ?2 AND id != ?3",
        )?
        .query_row(duckdb::params![website_id, name, exclude_id], |row| {
            row.get(0)
        })?
    } else {
        conn.prepare("SELECT COUNT(*) FROM saved_reports WHERE website_id = ?1 AND name = ?2")?
            .query_row(duckdb::params![website_id, name], |row| row.get(0))?
    };
    Ok(exists > 0)
}

pub async fn create_report_inner(
    db: &DuckDbBackend,
    website_id: &str,
    req: CreateReportRequest,
) -> Result<SavedReport> {
    let conn = db.conn.lock().await;

    let reports_count: i64 = conn
        .prepare("SELECT COUNT(*) FROM saved_reports WHERE website_id = ?1")?
        .query_row(duckdb::params![website_id], |row| row.get(0))?;
    if reports_count >= MAX_REPORTS_PER_WEBSITE {
        return Err(anyhow!("limit_exceeded"));
    }

    let duplicate_name_count: i64 = conn
        .prepare("SELECT COUNT(*) FROM saved_reports WHERE website_id = ?1 AND name = ?2")?
        .query_row(duckdb::params![website_id, &req.name], |row| row.get(0))?;
    if duplicate_name_count > 0 {
        return Err(anyhow!("duplicate_name"));
    }

    let id = generate_report_id();
    let config_json = serde_json::to_string(&req.config)?;
    conn.execute(
        r#"
        INSERT INTO saved_reports (
            id,
            website_id,
            name,
            description,
            config_json,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
        duckdb::params![id, website_id, req.name, req.description, config_json],
    )?;

    get_report_by_id(&conn, website_id, &id)?.ok_or_else(|| anyhow!("insert_failed"))
}

pub async fn update_report_inner(
    db: &DuckDbBackend,
    website_id: &str,
    report_id: &str,
    req: UpdateReportRequest,
) -> Result<Option<SavedReport>> {
    let conn = db.conn.lock().await;
    let Some(existing) = get_report_by_id(&conn, website_id, report_id)? else {
        return Ok(None);
    };

    let next_name = req.name.unwrap_or(existing.name);
    let next_description = req.description.unwrap_or(existing.description);
    let next_config = req.config.unwrap_or(existing.config);

    let duplicate_name_count: i64 = conn
        .prepare(
            "SELECT COUNT(*) FROM saved_reports WHERE website_id = ?1 AND name = ?2 AND id != ?3",
        )?
        .query_row(duckdb::params![website_id, &next_name, report_id], |row| {
            row.get(0)
        })?;
    if duplicate_name_count > 0 {
        return Err(anyhow!("duplicate_name"));
    }

    let config_json = serde_json::to_string(&next_config)?;
    conn.execute(
        r#"
        UPDATE saved_reports
        SET
            name = ?1,
            description = ?2,
            config_json = ?3,
            updated_at = CURRENT_TIMESTAMP
        WHERE website_id = ?4 AND id = ?5
        "#,
        duckdb::params![
            next_name,
            next_description,
            config_json,
            website_id,
            report_id
        ],
    )?;

    get_report_by_id(&conn, website_id, report_id)
}

pub async fn delete_report_inner(
    db: &DuckDbBackend,
    website_id: &str,
    report_id: &str,
) -> Result<bool> {
    let conn = db.conn.lock().await;
    let changed = conn.execute(
        "DELETE FROM saved_reports WHERE website_id = ?1 AND id = ?2",
        duckdb::params![website_id, report_id],
    )?;
    Ok(changed > 0)
}

pub async fn touch_report_last_run_inner(
    db: &DuckDbBackend,
    website_id: &str,
    report_id: &str,
) -> Result<()> {
    let conn = db.conn.lock().await;
    conn.execute(
        r#"
        UPDATE saved_reports
        SET
            last_run_at = CURRENT_TIMESTAMP,
            updated_at = CURRENT_TIMESTAMP
        WHERE website_id = ?1 AND id = ?2
        "#,
        duckdb::params![website_id, report_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config_reports_invalid_payload() {
        let err = parse_config("{not-json}");
        assert!(err.is_err());
    }

    #[test]
    fn report_type_round_trip() {
        let config = ReportConfig {
            report_type: ReportType::Metrics,
            ..ReportConfig::default()
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let parsed = parse_config(&json).expect("deserialize");
        assert_eq!(parsed.report_type, ReportType::Metrics);
    }

    #[test]
    fn report_type_parser_rejects_unknown_values() {
        let err = report_type_from_str("unknown");
        assert!(err.is_err());
    }
}
