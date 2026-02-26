use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, NaiveDateTime, Utc};
use ipnet::IpNet;
use rand::Rng;
use serde::{Deserialize, Serialize};

use sparklytics_core::analytics::{
    BotListEntry, BotMatchType, BotPolicy, BotPolicyAuditRecord, BotPolicyMode, BotReasonCount,
    BotRecomputeRun, BotRecomputeStatus, BotReport, BotReportSplit, BotReportTimeseriesPoint,
    BotReportTopUserAgent, BotSummary, CreateBotListEntryRequest, UpdateBotPolicyRequest,
};

use crate::DuckDbBackend;

const DEFAULT_POLICY_MODE: BotPolicyMode = BotPolicyMode::Balanced;
const DEFAULT_POLICY_THRESHOLD: i32 = 70;

fn generate_id(prefix: &str) -> String {
    let mut rng = rand::thread_rng();
    let chars: String = (0..16)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect();
    format!("{prefix}_{chars}")
}

fn policy_mode_to_str(mode: &BotPolicyMode) -> &'static str {
    match mode {
        BotPolicyMode::Strict => "strict",
        BotPolicyMode::Balanced => "balanced",
        BotPolicyMode::Off => "off",
    }
}

fn policy_mode_from_str(raw: &str) -> Result<BotPolicyMode> {
    match raw {
        "strict" => Ok(BotPolicyMode::Strict),
        "balanced" => Ok(BotPolicyMode::Balanced),
        "off" => Ok(BotPolicyMode::Off),
        _ => Err(anyhow!("invalid_bot_policy_mode")),
    }
}

fn match_type_to_str(match_type: &BotMatchType) -> &'static str {
    match match_type {
        BotMatchType::UaContains => "ua_contains",
        BotMatchType::IpExact => "ip_exact",
        BotMatchType::IpCidr => "ip_cidr",
    }
}

fn match_type_from_str(raw: &str) -> Result<BotMatchType> {
    match raw {
        "ua_contains" => Ok(BotMatchType::UaContains),
        "ip_exact" => Ok(BotMatchType::IpExact),
        "ip_cidr" => Ok(BotMatchType::IpCidr),
        _ => Err(anyhow!("invalid_bot_match_type")),
    }
}

fn recompute_status_to_str(status: &BotRecomputeStatus) -> &'static str {
    match status {
        BotRecomputeStatus::Queued => "queued",
        BotRecomputeStatus::Running => "running",
        BotRecomputeStatus::Success => "success",
        BotRecomputeStatus::Failed => "failed",
    }
}

fn recompute_status_from_str(raw: &str) -> Result<BotRecomputeStatus> {
    match raw {
        "queued" => Ok(BotRecomputeStatus::Queued),
        "running" => Ok(BotRecomputeStatus::Running),
        "success" => Ok(BotRecomputeStatus::Success),
        "failed" => Ok(BotRecomputeStatus::Failed),
        _ => Err(anyhow!("invalid_bot_recompute_status")),
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CursorPayload {
    created_at: String,
    id: String,
}

fn encode_cursor(created_at: &str, id: &str) -> Result<String> {
    let payload = CursorPayload {
        created_at: created_at.to_string(),
        id: id.to_string(),
    };
    Ok(STANDARD.encode(serde_json::to_vec(&payload)?))
}

fn decode_cursor(cursor: &str) -> Result<CursorPayload> {
    let decoded = STANDARD
        .decode(cursor)
        .map_err(|_| anyhow!("invalid_cursor"))?;
    serde_json::from_slice::<CursorPayload>(&decoded).map_err(|_| anyhow!("invalid_cursor"))
}

fn parse_optional_json(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::json!({ "raw": raw }))
}

fn parse_datetime(raw: &str) -> Result<NaiveDateTime> {
    NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S"))
        .map_err(|_| anyhow!("invalid_datetime"))
}

fn default_policy(website_id: &str) -> BotPolicy {
    BotPolicy {
        website_id: website_id.to_string(),
        mode: DEFAULT_POLICY_MODE,
        threshold_score: DEFAULT_POLICY_THRESHOLD,
        updated_at: Utc::now().to_rfc3339(),
    }
}

impl DuckDbBackend {
    pub async fn get_bot_policy(&self, website_id: &str) -> Result<BotPolicy> {
        let conn = self.conn.lock().await;
        let row = conn
            .prepare(
                "SELECT mode, threshold_score, CAST(updated_at AS VARCHAR)
                 FROM bot_policies
                 WHERE website_id = ?1",
            )?
            .query_row(duckdb::params![website_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, String>(2)?,
                ))
            });

        match row {
            Ok((mode, threshold_score, updated_at)) => Ok(BotPolicy {
                website_id: website_id.to_string(),
                mode: policy_mode_from_str(&mode)?,
                threshold_score: threshold_score.clamp(0, 100),
                updated_at,
            }),
            Err(_) => Ok(default_policy(website_id)),
        }
    }

    pub async fn upsert_bot_policy(
        &self,
        website_id: &str,
        req: &UpdateBotPolicyRequest,
    ) -> Result<BotPolicy> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO bot_policies (website_id, mode, threshold_score, updated_at)
             VALUES (?1, ?2, ?3, now())
             ON CONFLICT (website_id) DO UPDATE
             SET mode = EXCLUDED.mode,
                 threshold_score = EXCLUDED.threshold_score,
                 updated_at = now()",
            duckdb::params![
                website_id,
                policy_mode_to_str(&req.mode),
                req.threshold_score.clamp(0, 100)
            ],
        )?;
        drop(conn);
        self.get_bot_policy(website_id).await
    }

    pub async fn list_bot_entries(
        &self,
        website_id: &str,
        list_kind: &str,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<(Vec<BotListEntry>, Option<String>)> {
        let table = match list_kind {
            "allow" => "bot_allowlist",
            "block" => "bot_blocklist",
            _ => return Err(anyhow!("invalid_list_kind")),
        };
        let bounded_limit = limit.clamp(1, 200) as i64;
        let conn = self.conn.lock().await;
        let mut entries = Vec::new();
        if let Some(raw_cursor) = cursor.as_deref() {
            let decoded = decode_cursor(raw_cursor)?;
            let sql = format!(
                "SELECT id, match_type, match_value, note, CAST(created_at AS VARCHAR)
                 FROM {table}
                 WHERE website_id = ?1
                   AND (created_at < CAST(?2 AS TIMESTAMP) OR (created_at = CAST(?2 AS TIMESTAMP) AND id < ?3))
                 ORDER BY created_at DESC, id DESC
                 LIMIT {}",
                bounded_limit + 1
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(
                duckdb::params![website_id, decoded.created_at, decoded.id],
                |row| {
                    let match_type: String = row.get(1)?;
                    Ok(BotListEntry {
                        id: row.get(0)?,
                        match_type: match_type_from_str(&match_type)
                            .map_err(|_| duckdb::Error::InvalidQuery)?,
                        match_value: row.get(2)?,
                        note: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                },
            )?;
            for row in rows {
                entries.push(row?);
            }
        } else {
            let sql = format!(
                "SELECT id, match_type, match_value, note, CAST(created_at AS VARCHAR)
                 FROM {table}
                 WHERE website_id = ?1
                 ORDER BY created_at DESC, id DESC
                 LIMIT {}",
                bounded_limit + 1
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(duckdb::params![website_id], |row| {
                let match_type: String = row.get(1)?;
                Ok(BotListEntry {
                    id: row.get(0)?,
                    match_type: match_type_from_str(&match_type)
                        .map_err(|_| duckdb::Error::InvalidQuery)?,
                    match_value: row.get(2)?,
                    note: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?;
            for row in rows {
                entries.push(row?);
            }
        }
        let has_more = entries.len() > bounded_limit as usize;
        if has_more {
            entries.pop();
        }
        let next_cursor = if has_more {
            entries
                .last()
                .map(|entry| encode_cursor(&entry.created_at, &entry.id))
                .transpose()?
        } else {
            None
        };

        Ok((entries, next_cursor))
    }

    pub async fn create_bot_entry(
        &self,
        website_id: &str,
        list_kind: &str,
        req: &CreateBotListEntryRequest,
    ) -> Result<BotListEntry> {
        let table = match list_kind {
            "allow" => "bot_allowlist",
            "block" => "bot_blocklist",
            _ => return Err(anyhow!("invalid_list_kind")),
        };
        let id_prefix = if list_kind == "allow" {
            "allow"
        } else {
            "block"
        };
        let id = generate_id(id_prefix);
        let conn = self.conn.lock().await;
        conn.execute(
            &format!(
                "INSERT INTO {table} (id, website_id, match_type, match_value, note, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, now())"
            ),
            duckdb::params![
                id,
                website_id,
                match_type_to_str(&req.match_type),
                req.match_value.trim(),
                req.note.as_ref().map(|note| note.trim())
            ],
        )?;

        let (entry_id, match_type, match_value, note, created_at): (
            String,
            String,
            String,
            Option<String>,
            String,
        ) = conn
            .prepare(&format!(
                "SELECT id, match_type, match_value, note, CAST(created_at AS VARCHAR)
                 FROM {table}
                 WHERE id = ?1"
            ))?
            .query_row(duckdb::params![id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?;

        Ok(BotListEntry {
            id: entry_id,
            match_type: match_type_from_str(&match_type)?,
            match_value,
            note,
            created_at,
        })
    }

    pub async fn delete_bot_entry(
        &self,
        website_id: &str,
        list_kind: &str,
        entry_id: &str,
    ) -> Result<bool> {
        let table = match list_kind {
            "allow" => "bot_allowlist",
            "block" => "bot_blocklist",
            _ => return Err(anyhow!("invalid_list_kind")),
        };
        let conn = self.conn.lock().await;
        let affected = conn.execute(
            &format!("DELETE FROM {table} WHERE website_id = ?1 AND id = ?2"),
            duckdb::params![website_id, entry_id],
        )?;
        Ok(affected > 0)
    }

    pub async fn add_bot_policy_audit(
        &self,
        website_id: &str,
        actor: &str,
        action: &str,
        payload: &serde_json::Value,
    ) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO bot_policy_audit (id, website_id, actor, action, payload, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, now())",
            duckdb::params![
                generate_id("bot_audit"),
                website_id,
                actor,
                action,
                payload.to_string(),
            ],
        )?;
        Ok(())
    }

    pub async fn list_bot_policy_audit(
        &self,
        website_id: &str,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<(Vec<BotPolicyAuditRecord>, Option<String>)> {
        let bounded_limit = limit.clamp(1, 200) as i64;
        let conn = self.conn.lock().await;
        let mut records = Vec::new();
        if let Some(raw_cursor) = cursor.as_deref() {
            let decoded = decode_cursor(raw_cursor)?;
            let sql = format!(
                "SELECT id, actor, action, payload, CAST(created_at AS VARCHAR)
                 FROM bot_policy_audit
                 WHERE website_id = ?1
                   AND (created_at < CAST(?2 AS TIMESTAMP) OR (created_at = CAST(?2 AS TIMESTAMP) AND id < ?3))
                 ORDER BY created_at DESC, id DESC
                 LIMIT {}",
                bounded_limit + 1
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(
                duckdb::params![website_id, decoded.created_at, decoded.id],
                |row| {
                    let payload_raw: String = row.get(3)?;
                    Ok(BotPolicyAuditRecord {
                        id: row.get(0)?,
                        actor: row.get(1)?,
                        action: row.get(2)?,
                        payload: parse_optional_json(&payload_raw),
                        created_at: row.get(4)?,
                    })
                },
            )?;
            for row in rows {
                records.push(row?);
            }
        } else {
            let sql = format!(
                "SELECT id, actor, action, payload, CAST(created_at AS VARCHAR)
                 FROM bot_policy_audit
                 WHERE website_id = ?1
                 ORDER BY created_at DESC, id DESC
                 LIMIT {}",
                bounded_limit + 1
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(duckdb::params![website_id], |row| {
                let payload_raw: String = row.get(3)?;
                Ok(BotPolicyAuditRecord {
                    id: row.get(0)?,
                    actor: row.get(1)?,
                    action: row.get(2)?,
                    payload: parse_optional_json(&payload_raw),
                    created_at: row.get(4)?,
                })
            })?;
            for row in rows {
                records.push(row?);
            }
        }
        let has_more = records.len() > bounded_limit as usize;
        if has_more {
            records.pop();
        }
        let next_cursor = if has_more {
            records
                .last()
                .map(|entry| encode_cursor(&entry.created_at, &entry.id))
                .transpose()?
        } else {
            None
        };

        Ok((records, next_cursor))
    }

    pub async fn classify_override_for_request(
        &self,
        website_id: &str,
        source_ip: &str,
        user_agent: &str,
    ) -> Result<Option<bool>> {
        let conn = self.conn.lock().await;
        let parsed_ip = source_ip.parse::<std::net::IpAddr>().ok();
        let ua = user_agent.to_ascii_lowercase();

        let matcher = |match_type: &str, match_value: &str| -> bool {
            let value = match_value.trim();
            match match_type {
                "ua_contains" => ua.contains(&value.to_ascii_lowercase()),
                "ip_exact" => parsed_ip
                    .map(|ip| ip.to_string().eq_ignore_ascii_case(value))
                    .unwrap_or(false),
                "ip_cidr" => match (parsed_ip, value.parse::<IpNet>().ok()) {
                    (Some(ip), Some(net)) => net.contains(&ip),
                    _ => false,
                },
                _ => false,
            }
        };

        let mut block_stmt = conn.prepare(
            "SELECT match_type, match_value
             FROM bot_blocklist
             WHERE website_id = ?1",
        )?;
        let block_rows = block_stmt.query_map(duckdb::params![website_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in block_rows {
            let (match_type, match_value) = row?;
            if matcher(&match_type, &match_value) {
                return Ok(Some(true));
            }
        }

        let mut allow_stmt = conn.prepare(
            "SELECT match_type, match_value
             FROM bot_allowlist
             WHERE website_id = ?1",
        )?;
        let allow_rows = allow_stmt.query_map(duckdb::params![website_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in allow_rows {
            let (match_type, match_value) = row?;
            if matcher(&match_type, &match_value) {
                return Ok(Some(false));
            }
        }
        Ok(None)
    }

    pub async fn list_bot_override_rules(
        &self,
        website_id: &str,
    ) -> Result<(Vec<(BotMatchType, String)>, Vec<(BotMatchType, String)>)> {
        let conn = self.conn.lock().await;

        let mut block_rules = Vec::new();
        let mut block_stmt = conn.prepare(
            "SELECT match_type, match_value
             FROM bot_blocklist
             WHERE website_id = ?1",
        )?;
        let block_rows = block_stmt.query_map(duckdb::params![website_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in block_rows {
            let (match_type, match_value) = row?;
            block_rules.push((match_type_from_str(&match_type)?, match_value));
        }

        let mut allow_rules = Vec::new();
        let mut allow_stmt = conn.prepare(
            "SELECT match_type, match_value
             FROM bot_allowlist
             WHERE website_id = ?1",
        )?;
        let allow_rows = allow_stmt.query_map(duckdb::params![website_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in allow_rows {
            let (match_type, match_value) = row?;
            allow_rules.push((match_type_from_str(&match_type)?, match_value));
        }

        Ok((block_rules, allow_rules))
    }

    pub async fn get_bot_summary(
        &self,
        website_id: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<BotSummary> {
        let conn = self.conn.lock().await;
        let start = start_date.format("%Y-%m-%d %H:%M:%S%.f").to_string();
        let end = end_date.format("%Y-%m-%d %H:%M:%S%.f").to_string();

        let (bot_events, human_events): (i64, i64) = conn
            .prepare(
                "SELECT
                    COALESCE(SUM(CASE WHEN is_bot THEN 1 ELSE 0 END), 0) AS bot_events,
                    COALESCE(SUM(CASE WHEN is_bot THEN 0 ELSE 1 END), 0) AS human_events
                 FROM events
                 WHERE website_id = ?1
                   AND created_at >= ?2
                   AND created_at < ?3",
            )?
            .query_row(duckdb::params![website_id, start, end], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;

        let mut stmt = conn.prepare(
            "SELECT COALESCE(bot_reason, 'unknown') AS code, COUNT(*) AS count
             FROM events
             WHERE website_id = ?1
               AND created_at >= ?2
               AND created_at < ?3
               AND is_bot = TRUE
             GROUP BY code
             ORDER BY count DESC
             LIMIT 10",
        )?;
        let reason_rows = stmt.query_map(duckdb::params![website_id, start, end], |row| {
            Ok(BotReasonCount {
                code: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        let mut top_reasons = Vec::new();
        for row in reason_rows {
            top_reasons.push(row?);
        }

        let total = bot_events + human_events;
        let bot_rate = if total > 0 {
            bot_events as f64 / total as f64
        } else {
            0.0
        };

        Ok(BotSummary {
            website_id: website_id.to_string(),
            start_date: start_date.to_rfc3339(),
            end_date: end_date.to_rfc3339(),
            bot_events,
            human_events,
            bot_rate,
            top_reasons,
        })
    }

    pub async fn get_bot_report(
        &self,
        website_id: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        granularity: &str,
    ) -> Result<BotReport> {
        let summary = self
            .get_bot_summary(website_id, start_date, end_date)
            .await?;

        let conn = self.conn.lock().await;
        let start = start_date.format("%Y-%m-%d %H:%M:%S%.f").to_string();
        let end = end_date.format("%Y-%m-%d %H:%M:%S%.f").to_string();
        let bucket_sql = if granularity == "hour" {
            "date_trunc('hour', created_at)"
        } else {
            "date_trunc('day', created_at)"
        };

        let mut ts_stmt = conn.prepare(&format!(
            "SELECT
                CAST({bucket_sql} AS VARCHAR) AS period_start,
                SUM(CASE WHEN is_bot THEN 1 ELSE 0 END) AS bot_events,
                SUM(CASE WHEN is_bot THEN 0 ELSE 1 END) AS human_events
             FROM events
             WHERE website_id = ?1
               AND created_at >= ?2
               AND created_at < ?3
             GROUP BY period_start
             ORDER BY period_start ASC"
        ))?;
        let ts_rows = ts_stmt.query_map(duckdb::params![website_id, start, end], |row| {
            Ok(BotReportTimeseriesPoint {
                period_start: row.get(0)?,
                bot_events: row.get(1)?,
                human_events: row.get(2)?,
            })
        })?;
        let mut timeseries = Vec::new();
        for row in ts_rows {
            timeseries.push(row?);
        }

        let mut ua_stmt = conn.prepare(
            "SELECT user_agent, COUNT(*) AS count
             FROM events
             WHERE website_id = ?1
               AND created_at >= ?2
               AND created_at < ?3
               AND is_bot = TRUE
               AND user_agent IS NOT NULL
             GROUP BY user_agent
             ORDER BY count DESC
             LIMIT 10",
        )?;
        let ua_rows = ua_stmt.query_map(duckdb::params![website_id, start, end], |row| {
            Ok(BotReportTopUserAgent {
                value: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        let mut top_user_agents = Vec::new();
        for row in ua_rows {
            top_user_agents.push(row?);
        }

        Ok(BotReport {
            split: BotReportSplit {
                bot_events: summary.bot_events,
                human_events: summary.human_events,
                bot_rate: summary.bot_rate,
            },
            timeseries,
            top_reasons: summary.top_reasons,
            top_user_agents,
        })
    }

    pub async fn has_active_bot_recompute(&self, website_id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let count: i64 = conn
            .prepare(
                "SELECT COUNT(*) FROM bot_recompute_runs
                 WHERE website_id = ?1
                   AND status IN ('queued', 'running')",
            )?
            .query_row(duckdb::params![website_id], |row| row.get(0))?;
        Ok(count > 0)
    }

    pub async fn create_bot_recompute_run(
        &self,
        website_id: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<BotRecomputeRun> {
        let id = generate_id("bot_recompute");
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO bot_recompute_runs
                (id, website_id, start_date, end_date, status, created_at)
             VALUES
                (?1, ?2, ?3, ?4, 'queued', now())",
            duckdb::params![
                id,
                website_id,
                start_date.format("%Y-%m-%d %H:%M:%S%.f").to_string(),
                end_date.format("%Y-%m-%d %H:%M:%S%.f").to_string()
            ],
        )?;
        drop(conn);
        self.get_bot_recompute_run(website_id, &id)
            .await?
            .ok_or_else(|| anyhow!("bot_recompute_run_not_found"))
    }

    pub async fn get_bot_recompute_run(
        &self,
        website_id: &str,
        run_id: &str,
    ) -> Result<Option<BotRecomputeRun>> {
        let conn = self.conn.lock().await;
        let row = conn
            .prepare(
                "SELECT id, website_id, status, CAST(start_date AS VARCHAR), CAST(end_date AS VARCHAR),
                        CAST(created_at AS VARCHAR), CAST(started_at AS VARCHAR), CAST(completed_at AS VARCHAR), error_message
                 FROM bot_recompute_runs
                 WHERE website_id = ?1 AND id = ?2",
            )?
            .query_row(duckdb::params![website_id, run_id], |row| {
                Ok(BotRecomputeRun {
                    job_id: row.get(0)?,
                    website_id: row.get(1)?,
                    status: recompute_status_from_str(&row.get::<_, String>(2)?)
                        .map_err(|_| duckdb::Error::InvalidQuery)?,
                    start_date: row.get::<_, String>(3)?,
                    end_date: row.get::<_, String>(4)?,
                    created_at: row.get(5)?,
                    started_at: row.get(6)?,
                    completed_at: row.get(7)?,
                    error_message: row.get(8)?,
                })
            });
        match row {
            Ok(run) => Ok(Some(run)),
            Err(_) => Ok(None),
        }
    }

    pub async fn update_bot_recompute_status(
        &self,
        run_id: &str,
        status: BotRecomputeStatus,
        error_message: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().await;
        match status {
            BotRecomputeStatus::Running => {
                conn.execute(
                    "UPDATE bot_recompute_runs
                     SET status = ?2, started_at = now(), error_message = NULL
                     WHERE id = ?1",
                    duckdb::params![run_id, recompute_status_to_str(&status)],
                )?;
            }
            BotRecomputeStatus::Success | BotRecomputeStatus::Failed => {
                conn.execute(
                    "UPDATE bot_recompute_runs
                     SET status = ?2, completed_at = now(), error_message = ?3
                     WHERE id = ?1",
                    duckdb::params![run_id, recompute_status_to_str(&status), error_message],
                )?;
            }
            BotRecomputeStatus::Queued => {}
        }
        Ok(())
    }

    pub async fn list_events_for_recompute(
        &self,
        website_id: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        cursor: Option<(String, String)>,
        limit: u32,
    ) -> Result<(
        Vec<(String, String, String, Option<String>, Option<String>)>,
        Option<(String, String)>,
    )> {
        let bounded_limit = limit.clamp(1, 1000) as i64;
        let start = start_date.format("%Y-%m-%d %H:%M:%S%.f").to_string();
        let end = end_date.format("%Y-%m-%d %H:%M:%S%.f").to_string();
        let conn = self.conn.lock().await;

        let mut rows_with_cursor: Vec<(
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            String,
        )> = Vec::new();
        if let Some((cursor_created_at, cursor_id)) = cursor {
            let sql = format!(
                "SELECT id, visitor_id, url, source_ip, user_agent, CAST(created_at AS VARCHAR)
                 FROM events
                 WHERE website_id = ?1
                   AND created_at >= ?2
                   AND created_at < ?3
                   AND (created_at > CAST(?4 AS TIMESTAMP) OR (created_at = CAST(?4 AS TIMESTAMP) AND id > ?5))
                 ORDER BY created_at ASC, id ASC
                 LIMIT {}",
                bounded_limit + 1
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(
                duckdb::params![website_id, start, end, cursor_created_at, cursor_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )?;
            for row in rows {
                rows_with_cursor.push(row?);
            }
        } else {
            let sql = format!(
                "SELECT id, visitor_id, url, source_ip, user_agent, CAST(created_at AS VARCHAR)
                 FROM events
                 WHERE website_id = ?1
                   AND created_at >= ?2
                   AND created_at < ?3
                 ORDER BY created_at ASC, id ASC
                 LIMIT {}",
                bounded_limit + 1
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(duckdb::params![website_id, start, end], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            })?;
            for row in rows {
                rows_with_cursor.push(row?);
            }
        }

        let has_more = rows_with_cursor.len() > bounded_limit as usize;
        if has_more {
            rows_with_cursor.pop();
        }
        let next_cursor = rows_with_cursor
            .last()
            .map(|row| (row.5.clone(), row.0.clone()));
        let mut events = Vec::with_capacity(rows_with_cursor.len());
        for (event_id, visitor_id, url, source_ip, user_agent, _) in rows_with_cursor {
            events.push((event_id, visitor_id, url, source_ip, user_agent));
        }
        Ok((events, next_cursor))
    }

    pub async fn update_event_bot_classification(
        &self,
        event_id: &str,
        is_bot: bool,
        bot_score: i32,
        bot_reason: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE events
             SET is_bot = ?2, bot_score = ?3, bot_reason = ?4
             WHERE id = ?1",
            duckdb::params![event_id, is_bot, bot_score, bot_reason],
        )?;
        Ok(())
    }

    pub async fn recompute_sessions_bot_rollup_in_window(
        &self,
        website_id: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<()> {
        let start = start_date.format("%Y-%m-%d %H:%M:%S%.f").to_string();
        let end = end_date.format("%Y-%m-%d %H:%M:%S%.f").to_string();
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE sessions s
             SET is_bot = agg.is_bot,
                 bot_score = agg.bot_score,
                 bot_reason = agg.bot_reason
             FROM (
                WITH touched_sessions AS (
                    SELECT DISTINCT session_id
                    FROM events
                    WHERE website_id = ?1
                      AND created_at >= ?2
                      AND created_at < ?3
                )
                SELECT
                    e.session_id,
                    MAX(CASE WHEN e.is_bot THEN 1 ELSE 0 END) = 1 AS is_bot,
                    MAX(e.bot_score) AS bot_score,
                    MAX(e.bot_reason) AS bot_reason
                FROM events e
                INNER JOIN touched_sessions t ON t.session_id = e.session_id
                WHERE e.website_id = ?1
                GROUP BY e.session_id
             ) agg
             WHERE s.website_id = ?1
               AND s.session_id = agg.session_id",
            duckdb::params![website_id, start, end],
        )?;
        Ok(())
    }

    pub async fn effective_include_bots_default(&self, website_id: &str) -> Result<bool> {
        let policy = self.get_bot_policy(website_id).await?;
        Ok(matches!(policy.mode, BotPolicyMode::Off))
    }

    pub async fn parse_bot_recompute_window(
        &self,
        website_id: &str,
        run_id: &str,
    ) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>> {
        let Some(run) = self.get_bot_recompute_run(website_id, run_id).await? else {
            return Ok(None);
        };
        let start = parse_datetime(&run.start_date)?;
        let end = parse_datetime(&run.end_date)?;
        Ok(Some((
            DateTime::<Utc>::from_naive_utc_and_offset(start, Utc),
            DateTime::<Utc>::from_naive_utc_and_offset(end, Utc),
        )))
    }
}
