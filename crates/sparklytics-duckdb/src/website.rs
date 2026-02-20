use anyhow::Result;
use serde::Serialize;

use crate::DuckDbBackend;

#[derive(Debug, Clone, Serialize)]
pub struct Website {
    pub id: String,
    pub tenant_id: Option<String>,
    pub name: String,
    pub domain: String,
    pub timezone: String,
    pub share_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct CreateWebsiteParams {
    pub name: String,
    pub domain: String,
    pub timezone: Option<String>,
}

pub struct UpdateWebsiteParams {
    pub name: Option<String>,
    pub domain: Option<String>,
    pub timezone: Option<String>,
}

/// Generate a website ID: "site_" + 10 random alphanumeric chars.
fn generate_website_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let chars: String = (0..10)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect();
    format!("site_{}", chars)
}

impl DuckDbBackend {
    pub async fn create_website(&self, params: CreateWebsiteParams) -> Result<Website> {
        let conn = self.conn.lock().await;
        let id = generate_website_id();
        let timezone = params.timezone.unwrap_or_else(|| "UTC".to_string());

        conn.execute(
            "INSERT INTO websites (id, tenant_id, name, domain, timezone, created_at, updated_at) \
             VALUES (?1, NULL, ?2, ?3, ?4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            duckdb::params![id, params.name, params.domain, timezone],
        )?;

        // Read back the created row to get timestamps.
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, name, domain, timezone, share_id, CAST(created_at AS VARCHAR), CAST(updated_at AS VARCHAR) \
             FROM websites WHERE id = ?1",
        )?;
        let website = stmt.query_row(duckdb::params![id], |row| {
            Ok(Website {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                name: row.get(2)?,
                domain: row.get(3)?,
                timezone: row.get(4)?,
                share_id: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;

        Ok(website)
    }

    pub async fn list_websites(
        &self,
        limit: i64,
        cursor: Option<&str>,
    ) -> Result<(Vec<Website>, i64, bool)> {
        let conn = self.conn.lock().await;

        // Total count.
        let total: i64 = conn
            .prepare("SELECT COUNT(*) FROM websites")?
            .query_row([], |row| row.get(0))?;

        let (sql, params): (String, Vec<Box<dyn duckdb::types::ToSql>>) = if let Some(cursor) =
            cursor
        {
            (
                "SELECT id, tenant_id, name, domain, timezone, share_id, CAST(created_at AS VARCHAR), CAST(updated_at AS VARCHAR) \
                 FROM websites WHERE id > ?1 ORDER BY id LIMIT ?2"
                    .to_string(),
                vec![
                    Box::new(cursor.to_string()) as Box<dyn duckdb::types::ToSql>,
                    Box::new(limit),
                ],
            )
        } else {
            (
                "SELECT id, tenant_id, name, domain, timezone, share_id, CAST(created_at AS VARCHAR), CAST(updated_at AS VARCHAR) \
                 FROM websites ORDER BY id LIMIT ?1"
                    .to_string(),
                vec![Box::new(limit) as Box<dyn duckdb::types::ToSql>],
            )
        };

        let param_refs: Vec<&dyn duckdb::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(Website {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                name: row.get(2)?,
                domain: row.get(3)?,
                timezone: row.get(4)?,
                share_id: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;

        let mut websites = Vec::new();
        for row in rows {
            websites.push(row?);
        }

        let has_more = if let Some(last) = websites.last() {
            let remaining: i64 = conn
                .prepare("SELECT COUNT(*) FROM websites WHERE id > ?1")?
                .query_row(duckdb::params![last.id], |row| row.get(0))?;
            remaining > 0
        } else {
            false
        };

        Ok((websites, total, has_more))
    }

    pub async fn get_website(&self, id: &str) -> Result<Option<Website>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, tenant_id, name, domain, timezone, share_id, CAST(created_at AS VARCHAR), CAST(updated_at AS VARCHAR) \
             FROM websites WHERE id = ?1",
        )?;
        let result = stmt
            .query_row(duckdb::params![id], |row| {
                Ok(Website {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    name: row.get(2)?,
                    domain: row.get(3)?,
                    timezone: row.get(4)?,
                    share_id: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })
            .ok();
        Ok(result)
    }

    pub async fn update_website(
        &self,
        id: &str,
        params: UpdateWebsiteParams,
    ) -> Result<Option<Website>> {
        let conn = self.conn.lock().await;

        // Check existence first.
        let exists: i64 = conn
            .prepare("SELECT COUNT(*) FROM websites WHERE id = ?1")?
            .query_row(duckdb::params![id], |row| row.get(0))?;
        if exists == 0 {
            return Ok(None);
        }

        if let Some(ref name) = params.name {
            conn.execute(
                "UPDATE websites SET name = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                duckdb::params![name, id],
            )?;
        }
        if let Some(ref domain) = params.domain {
            conn.execute(
                "UPDATE websites SET domain = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                duckdb::params![domain, id],
            )?;
        }
        if let Some(ref timezone) = params.timezone {
            conn.execute(
                "UPDATE websites SET timezone = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                duckdb::params![timezone, id],
            )?;
        }

        // Read back updated row.
        let website = conn
            .prepare(
                "SELECT id, tenant_id, name, domain, timezone, share_id, CAST(created_at AS VARCHAR), CAST(updated_at AS VARCHAR) \
                 FROM websites WHERE id = ?1",
            )?
            .query_row(duckdb::params![id], |row| {
                Ok(Website {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    name: row.get(2)?,
                    domain: row.get(3)?,
                    timezone: row.get(4)?,
                    share_id: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })?;

        Ok(Some(website))
    }

    /// Delete a website and all associated data.
    ///
    /// CLAUDE.md critical fact #16: DuckDB doesn't enforce FKs — cascade deletes manually.
    /// Order: events → sessions → website.
    pub async fn delete_website(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;

        let exists: i64 = conn
            .prepare("SELECT COUNT(*) FROM websites WHERE id = ?1")?
            .query_row(duckdb::params![id], |row| row.get(0))?;
        if exists == 0 {
            return Ok(false);
        }

        // Cascade delete: events → sessions → website (critical fact #16).
        conn.execute(
            "DELETE FROM events WHERE website_id = ?1",
            duckdb::params![id],
        )?;
        conn.execute(
            "DELETE FROM sessions WHERE website_id = ?1",
            duckdb::params![id],
        )?;
        conn.execute("DELETE FROM websites WHERE id = ?1", duckdb::params![id])?;

        Ok(true)
    }
}
