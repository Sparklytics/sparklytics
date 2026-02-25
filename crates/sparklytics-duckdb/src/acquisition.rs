use anyhow::Result;
use rand::Rng;

use sparklytics_core::analytics::{
    CampaignLink, CreateCampaignLinkRequest, CreateTrackingPixelRequest, LinkStatsResponse,
    PixelStatsResponse, TrackingPixel, UpdateCampaignLinkRequest, UpdateTrackingPixelRequest,
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

fn generate_link_id() -> String {
    format!("lnk_{}", random_alnum(21))
}

fn generate_slug() -> String {
    random_alnum(8)
}

fn generate_pixel_id() -> String {
    format!("pxl_{}", random_alnum(21))
}

fn generate_pixel_key() -> String {
    format!("px_{}", random_alnum(22))
}

fn resolve_unique_value<F>(
    conn: &duckdb::Connection,
    exists_sql: &str,
    mut candidate: String,
    max_retries: usize,
    mut next_candidate: F,
) -> Result<String>
where
    F: FnMut() -> String,
{
    for _ in 0..max_retries {
        let exists: i64 = conn
            .prepare(exists_sql)?
            .query_row(duckdb::params![candidate.as_str()], |row| row.get(0))?;
        if exists == 0 {
            return Ok(candidate);
        }
        candidate = next_candidate();
    }
    Ok(candidate)
}

fn map_campaign_link_row(row: &duckdb::Row<'_>) -> Result<CampaignLink, duckdb::Error> {
    Ok(CampaignLink {
        id: row.get(0)?,
        website_id: row.get(1)?,
        name: row.get(2)?,
        slug: row.get(3)?,
        destination_url: row.get(4)?,
        utm_source: row.get(5)?,
        utm_medium: row.get(6)?,
        utm_campaign: row.get(7)?,
        utm_term: row.get(8)?,
        utm_content: row.get(9)?,
        is_active: row.get(10)?,
        created_at: row.get(11)?,
        clicks: None,
        unique_visitors: None,
        conversions: None,
        revenue: None,
    })
}

fn map_tracking_pixel_row(row: &duckdb::Row<'_>) -> Result<TrackingPixel, duckdb::Error> {
    Ok(TrackingPixel {
        id: row.get(0)?,
        website_id: row.get(1)?,
        name: row.get(2)?,
        pixel_key: row.get(3)?,
        default_url: row.get(4)?,
        is_active: row.get(5)?,
        created_at: row.get(6)?,
        views: None,
        unique_visitors: None,
    })
}

impl DuckDbBackend {
    pub async fn create_campaign_link(
        &self,
        website_id: &str,
        req: CreateCampaignLinkRequest,
    ) -> Result<CampaignLink> {
        let conn = self.conn.lock().await;
        let id = generate_link_id();
        let slug = resolve_unique_value(
            &conn,
            "SELECT COUNT(*) FROM campaign_links WHERE slug = ?1",
            generate_slug(),
            8,
            generate_slug,
        )?;

        conn.execute(
            r#"
            INSERT INTO campaign_links (
                id, website_id, name, slug, destination_url,
                utm_source, utm_medium, utm_campaign, utm_term, utm_content,
                is_active, created_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9, ?10,
                TRUE, CURRENT_TIMESTAMP
            )
            "#,
            duckdb::params![
                id,
                website_id,
                req.name,
                slug,
                req.destination_url,
                req.utm_source,
                req.utm_medium,
                req.utm_campaign,
                req.utm_term,
                req.utm_content
            ],
        )?;

        let mut stmt = conn.prepare(
            r#"
            SELECT
                id, website_id, name, slug, destination_url,
                utm_source, utm_medium, utm_campaign, utm_term, utm_content,
                is_active, CAST(created_at AS VARCHAR)
            FROM campaign_links
            WHERE id = ?1
            "#,
        )?;
        let link = stmt.query_row(duckdb::params![id], map_campaign_link_row)?;
        Ok(link)
    }

    pub async fn list_campaign_links(&self, website_id: &str) -> Result<Vec<CampaignLink>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id, website_id, name, slug, destination_url,
                utm_source, utm_medium, utm_campaign, utm_term, utm_content,
                is_active, CAST(created_at AS VARCHAR)
            FROM campaign_links
            WHERE website_id = ?1
            ORDER BY created_at DESC, id DESC
            "#,
        )?;
        let rows = stmt.query_map(duckdb::params![website_id], map_campaign_link_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub async fn list_campaign_links_with_stats(&self, website_id: &str) -> Result<Vec<CampaignLink>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            WITH click_stats AS (
                SELECT
                    link_id,
                    COUNT(*) AS clicks,
                    COUNT(DISTINCT visitor_id) AS unique_visitors
                FROM events
                WHERE website_id = ?1
                  AND event_name = 'link_click'
                  AND link_id IS NOT NULL
                GROUP BY link_id
            ),
            conversion_stats AS (
                SELECT
                    link_id,
                    COUNT(*) AS conversions,
                    COALESCE(SUM(TRY_CAST(json_extract_string(event_data, '$.value') AS DOUBLE)), 0.0) AS revenue
                FROM events
                WHERE website_id = ?1
                  AND event_name = 'goal_conversion'
                  AND link_id IS NOT NULL
                GROUP BY link_id
            )
            SELECT
                l.id,
                l.website_id,
                l.name,
                l.slug,
                l.destination_url,
                l.utm_source,
                l.utm_medium,
                l.utm_campaign,
                l.utm_term,
                l.utm_content,
                l.is_active,
                CAST(l.created_at AS VARCHAR),
                COALESCE(cs.clicks, 0) AS clicks,
                COALESCE(cs.unique_visitors, 0) AS unique_visitors,
                COALESCE(cvs.conversions, 0) AS conversions,
                COALESCE(cvs.revenue, 0.0) AS revenue
            FROM campaign_links l
            LEFT JOIN click_stats cs
              ON cs.link_id = l.id
            LEFT JOIN conversion_stats cvs
              ON cvs.link_id = l.id
            WHERE l.website_id = ?1
            ORDER BY l.created_at DESC, l.id DESC
            "#,
        )?;
        let rows = stmt.query_map(duckdb::params![website_id], |row| {
            Ok(CampaignLink {
                id: row.get(0)?,
                website_id: row.get(1)?,
                name: row.get(2)?,
                slug: row.get(3)?,
                destination_url: row.get(4)?,
                utm_source: row.get(5)?,
                utm_medium: row.get(6)?,
                utm_campaign: row.get(7)?,
                utm_term: row.get(8)?,
                utm_content: row.get(9)?,
                is_active: row.get(10)?,
                created_at: row.get(11)?,
                clicks: Some(row.get(12)?),
                unique_visitors: Some(row.get(13)?),
                conversions: Some(row.get(14)?),
                revenue: Some(row.get(15)?),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub async fn get_campaign_link(
        &self,
        website_id: &str,
        link_id: &str,
    ) -> Result<Option<CampaignLink>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id, website_id, name, slug, destination_url,
                utm_source, utm_medium, utm_campaign, utm_term, utm_content,
                is_active, CAST(created_at AS VARCHAR)
            FROM campaign_links
            WHERE website_id = ?1 AND id = ?2
            "#,
        )?;
        let link = stmt
            .query_row(duckdb::params![website_id, link_id], map_campaign_link_row)
            .ok();
        Ok(link)
    }

    pub async fn get_campaign_link_by_slug(&self, slug: &str) -> Result<Option<CampaignLink>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id, website_id, name, slug, destination_url,
                utm_source, utm_medium, utm_campaign, utm_term, utm_content,
                is_active, CAST(created_at AS VARCHAR)
            FROM campaign_links
            WHERE slug = ?1
            "#,
        )?;
        let link = stmt
            .query_row(duckdb::params![slug], map_campaign_link_row)
            .ok();
        Ok(link)
    }

    pub async fn update_campaign_link(
        &self,
        website_id: &str,
        link_id: &str,
        req: UpdateCampaignLinkRequest,
    ) -> Result<Option<CampaignLink>> {
        let existing = self.get_campaign_link(website_id, link_id).await?;
        let Some(existing) = existing else {
            return Ok(None);
        };

        let name = req.name.unwrap_or(existing.name);
        let destination_url = req.destination_url.unwrap_or(existing.destination_url);
        let utm_source = req.utm_source.unwrap_or(existing.utm_source);
        let utm_medium = req.utm_medium.unwrap_or(existing.utm_medium);
        let utm_campaign = req.utm_campaign.unwrap_or(existing.utm_campaign);
        let utm_term = req.utm_term.unwrap_or(existing.utm_term);
        let utm_content = req.utm_content.unwrap_or(existing.utm_content);
        let is_active = req.is_active.unwrap_or(existing.is_active);

        let conn = self.conn.lock().await;
        conn.execute(
            r#"
            UPDATE campaign_links
            SET
                name = ?1,
                destination_url = ?2,
                utm_source = ?3,
                utm_medium = ?4,
                utm_campaign = ?5,
                utm_term = ?6,
                utm_content = ?7,
                is_active = ?8
            WHERE website_id = ?9 AND id = ?10
            "#,
            duckdb::params![
                name,
                destination_url,
                utm_source,
                utm_medium,
                utm_campaign,
                utm_term,
                utm_content,
                is_active,
                website_id,
                link_id
            ],
        )?;
        drop(conn);

        self.get_campaign_link(website_id, link_id).await
    }

    pub async fn delete_campaign_link(&self, website_id: &str, link_id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let changed = conn.execute(
            "DELETE FROM campaign_links WHERE website_id = ?1 AND id = ?2",
            duckdb::params![website_id, link_id],
        )?;
        Ok(changed > 0)
    }

    pub async fn get_campaign_link_stats(
        &self,
        website_id: &str,
        link_id: &str,
    ) -> Result<LinkStatsResponse> {
        let conn = self.conn.lock().await;

        let (clicks, unique_visitors, conversions, revenue): (i64, i64, i64, f64) = conn
            .prepare(
                r#"
                WITH click_stats AS (
                    SELECT
                        COUNT(*) AS clicks,
                        COUNT(DISTINCT visitor_id) AS unique_visitors
                    FROM events
                    WHERE website_id = ?1
                      AND event_name = 'link_click'
                      AND link_id = ?2
                ),
                conversion_stats AS (
                    SELECT
                        COUNT(*) AS conversions,
                        COALESCE(SUM(TRY_CAST(json_extract_string(event_data, '$.value') AS DOUBLE)), 0.0) AS revenue
                    FROM events
                    WHERE website_id = ?1
                      AND event_name = 'goal_conversion'
                      AND link_id = ?2
                )
                SELECT
                    COALESCE((SELECT clicks FROM click_stats), 0),
                    COALESCE((SELECT unique_visitors FROM click_stats), 0),
                    COALESCE((SELECT conversions FROM conversion_stats), 0),
                    COALESCE((SELECT revenue FROM conversion_stats), 0.0)
                "#,
            )?
            .query_row(duckdb::params![website_id, link_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?;

        Ok(LinkStatsResponse {
            link_id: link_id.to_string(),
            clicks,
            unique_visitors,
            conversions,
            revenue,
        })
    }

    pub async fn create_tracking_pixel(
        &self,
        website_id: &str,
        req: CreateTrackingPixelRequest,
    ) -> Result<TrackingPixel> {
        let conn = self.conn.lock().await;
        let id = generate_pixel_id();
        let pixel_key = resolve_unique_value(
            &conn,
            "SELECT COUNT(*) FROM tracking_pixels WHERE pixel_key = ?1",
            generate_pixel_key(),
            8,
            generate_pixel_key,
        )?;

        conn.execute(
            r#"
            INSERT INTO tracking_pixels (
                id, website_id, name, pixel_key, default_url, is_active, created_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, TRUE, CURRENT_TIMESTAMP
            )
            "#,
            duckdb::params![id, website_id, req.name, pixel_key, req.default_url],
        )?;

        let mut stmt = conn.prepare(
            r#"
            SELECT
                id, website_id, name, pixel_key, default_url, is_active, CAST(created_at AS VARCHAR)
            FROM tracking_pixels
            WHERE id = ?1
            "#,
        )?;
        let pixel = stmt.query_row(duckdb::params![id], map_tracking_pixel_row)?;
        Ok(pixel)
    }

    pub async fn list_tracking_pixels(&self, website_id: &str) -> Result<Vec<TrackingPixel>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id, website_id, name, pixel_key, default_url, is_active, CAST(created_at AS VARCHAR)
            FROM tracking_pixels
            WHERE website_id = ?1
            ORDER BY created_at DESC, id DESC
            "#,
        )?;
        let rows = stmt.query_map(duckdb::params![website_id], map_tracking_pixel_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub async fn list_tracking_pixels_with_stats(
        &self,
        website_id: &str,
    ) -> Result<Vec<TrackingPixel>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            WITH view_stats AS (
                SELECT
                    pixel_id,
                    COUNT(*) AS views,
                    COUNT(DISTINCT visitor_id) AS unique_visitors
                FROM events
                WHERE website_id = ?1
                  AND event_name = 'pixel_view'
                  AND pixel_id IS NOT NULL
                GROUP BY pixel_id
            )
            SELECT
                p.id,
                p.website_id,
                p.name,
                p.pixel_key,
                p.default_url,
                p.is_active,
                CAST(p.created_at AS VARCHAR),
                COALESCE(vs.views, 0) AS views,
                COALESCE(vs.unique_visitors, 0) AS unique_visitors
            FROM tracking_pixels p
            LEFT JOIN view_stats vs
              ON vs.pixel_id = p.id
            WHERE p.website_id = ?1
            ORDER BY p.created_at DESC, p.id DESC
            "#,
        )?;
        let rows = stmt.query_map(duckdb::params![website_id], |row| {
            Ok(TrackingPixel {
                id: row.get(0)?,
                website_id: row.get(1)?,
                name: row.get(2)?,
                pixel_key: row.get(3)?,
                default_url: row.get(4)?,
                is_active: row.get(5)?,
                created_at: row.get(6)?,
                views: Some(row.get(7)?),
                unique_visitors: Some(row.get(8)?),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub async fn get_tracking_pixel(
        &self,
        website_id: &str,
        pixel_id: &str,
    ) -> Result<Option<TrackingPixel>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id, website_id, name, pixel_key, default_url, is_active, CAST(created_at AS VARCHAR)
            FROM tracking_pixels
            WHERE website_id = ?1 AND id = ?2
            "#,
        )?;
        let pixel = stmt
            .query_row(duckdb::params![website_id, pixel_id], map_tracking_pixel_row)
            .ok();
        Ok(pixel)
    }

    pub async fn get_tracking_pixel_by_key(&self, pixel_key: &str) -> Result<Option<TrackingPixel>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id, website_id, name, pixel_key, default_url, is_active, CAST(created_at AS VARCHAR)
            FROM tracking_pixels
            WHERE pixel_key = ?1
            "#,
        )?;
        let pixel = stmt
            .query_row(duckdb::params![pixel_key], map_tracking_pixel_row)
            .ok();
        Ok(pixel)
    }

    pub async fn update_tracking_pixel(
        &self,
        website_id: &str,
        pixel_id: &str,
        req: UpdateTrackingPixelRequest,
    ) -> Result<Option<TrackingPixel>> {
        let existing = self.get_tracking_pixel(website_id, pixel_id).await?;
        let Some(existing) = existing else {
            return Ok(None);
        };

        let name = req.name.unwrap_or(existing.name);
        let default_url = req.default_url.unwrap_or(existing.default_url);
        let is_active = req.is_active.unwrap_or(existing.is_active);

        let conn = self.conn.lock().await;
        conn.execute(
            r#"
            UPDATE tracking_pixels
            SET
                name = ?1,
                default_url = ?2,
                is_active = ?3
            WHERE website_id = ?4 AND id = ?5
            "#,
            duckdb::params![name, default_url, is_active, website_id, pixel_id],
        )?;
        drop(conn);

        self.get_tracking_pixel(website_id, pixel_id).await
    }

    pub async fn delete_tracking_pixel(&self, website_id: &str, pixel_id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let changed = conn.execute(
            "DELETE FROM tracking_pixels WHERE website_id = ?1 AND id = ?2",
            duckdb::params![website_id, pixel_id],
        )?;
        Ok(changed > 0)
    }

    pub async fn get_tracking_pixel_stats(
        &self,
        website_id: &str,
        pixel_id: &str,
    ) -> Result<PixelStatsResponse> {
        let conn = self.conn.lock().await;
        let (views, unique_visitors): (i64, i64) = conn
            .prepare(
                r#"
                SELECT
                    COUNT(*) AS views,
                    COUNT(DISTINCT visitor_id) AS unique_visitors
                FROM events
                WHERE website_id = ?1
                  AND event_name = 'pixel_view'
                  AND pixel_id = ?2
                "#,
            )?
            .query_row(duckdb::params![website_id, pixel_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;

        Ok(PixelStatsResponse {
            pixel_id: pixel_id.to_string(),
            views,
            unique_visitors,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use sparklytics_core::event::Event;

    use super::*;

    fn sample_event(
        website_id: &str,
        session_id: &str,
        visitor_id: &str,
        event_name: &str,
        event_data: Option<String>,
        link_id: Option<String>,
        pixel_id: Option<String>,
    ) -> Event {
        Event {
            id: uuid::Uuid::new_v4().to_string(),
            website_id: website_id.to_string(),
            tenant_id: None,
            session_id: session_id.to_string(),
            visitor_id: visitor_id.to_string(),
            event_type: "event".to_string(),
            url: "https://example.com/page".to_string(),
            referrer_url: None,
            referrer_domain: None,
            event_name: Some(event_name.to_string()),
            event_data,
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
            link_id,
            pixel_id,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn campaign_link_stats_are_aggregated() {
        let db = DuckDbBackend::open_in_memory().expect("db");
        db.seed_website("site_test", "example.com")
            .await
            .expect("seed");

        let link = db
            .create_campaign_link(
                "site_test",
                CreateCampaignLinkRequest {
                    name: "Newsletter".to_string(),
                    destination_url: "https://example.com/pricing".to_string(),
                    utm_source: Some("newsletter".to_string()),
                    utm_medium: Some("email".to_string()),
                    utm_campaign: Some("spring".to_string()),
                    utm_term: None,
                    utm_content: None,
                },
            )
            .await
            .expect("create link");

        let click = sample_event(
            "site_test",
            "sess_1",
            "visitor_1",
            "link_click",
            None,
            Some(link.id.clone()),
            None,
        );
        let conversion = sample_event(
            "site_test",
            "sess_1",
            "visitor_1",
            "goal_conversion",
            Some(r#"{"value":"49.99"}"#.to_string()),
            Some(link.id.clone()),
            None,
        );
        db.insert_events(&[click, conversion]).await.expect("insert");

        let stats = db
            .get_campaign_link_stats("site_test", &link.id)
            .await
            .expect("stats");
        assert_eq!(stats.clicks, 1);
        assert_eq!(stats.unique_visitors, 1);
        assert_eq!(stats.conversions, 1);
        assert!((stats.revenue - 49.99).abs() < 0.001);

        let list = db
            .list_campaign_links_with_stats("site_test")
            .await
            .expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].clicks, Some(1));
    }

    #[tokio::test]
    async fn tracking_pixel_stats_are_aggregated() {
        let db = DuckDbBackend::open_in_memory().expect("db");
        db.seed_website("site_test", "example.com")
            .await
            .expect("seed");

        let pixel = db
            .create_tracking_pixel(
                "site_test",
                CreateTrackingPixelRequest {
                    name: "Email pixel".to_string(),
                    default_url: Some("https://example.com/docs".to_string()),
                },
            )
            .await
            .expect("create pixel");

        let view_one = sample_event(
            "site_test",
            "sess_1",
            "visitor_1",
            "pixel_view",
            None,
            None,
            Some(pixel.id.clone()),
        );
        let view_two = sample_event(
            "site_test",
            "sess_2",
            "visitor_2",
            "pixel_view",
            None,
            None,
            Some(pixel.id.clone()),
        );
        db.insert_events(&[view_one, view_two]).await.expect("insert");

        let stats = db
            .get_tracking_pixel_stats("site_test", &pixel.id)
            .await
            .expect("stats");
        assert_eq!(stats.views, 2);
        assert_eq!(stats.unique_visitors, 2);

        let list = db
            .list_tracking_pixels_with_stats("site_test")
            .await
            .expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].views, Some(2));
    }

    #[tokio::test]
    async fn campaign_link_slug_uniqueness_is_enforced() {
        let db = DuckDbBackend::open_in_memory().expect("db");
        db.seed_website("site_test", "example.com")
            .await
            .expect("seed");

        let link = db
            .create_campaign_link(
                "site_test",
                CreateCampaignLinkRequest {
                    name: "Primary".to_string(),
                    destination_url: "https://example.com/pricing".to_string(),
                    utm_source: None,
                    utm_medium: None,
                    utm_campaign: None,
                    utm_term: None,
                    utm_content: None,
                },
            )
            .await
            .expect("create");

        let conn = db.conn_for_test().await;
        let duplicate = conn.execute(
            r#"
            INSERT INTO campaign_links (
                id, website_id, name, slug, destination_url, is_active, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, TRUE, CURRENT_TIMESTAMP)
            "#,
            duckdb::params![
                "lnk_duplicate",
                "site_test",
                "Duplicate",
                link.slug,
                "https://example.com/docs"
            ],
        );
        assert!(duplicate.is_err());
    }

    #[tokio::test]
    async fn campaign_link_slug_collision_retries_to_next_candidate() {
        let db = DuckDbBackend::open_in_memory().expect("db");
        db.seed_website("site_test", "example.com")
            .await
            .expect("seed");

        let conn = db.conn_for_test().await;
        conn.execute(
            r#"
            INSERT INTO campaign_links (
                id, website_id, name, slug, destination_url, is_active, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, TRUE, CURRENT_TIMESTAMP)
            "#,
            duckdb::params![
                "lnk_existing",
                "site_test",
                "Existing",
                "dup_slug",
                "https://example.com/existing"
            ],
        )
        .expect("insert existing");

        let mut candidates = vec!["dup_slug".to_string(), "unique_slug".to_string()].into_iter();
        let resolved = resolve_unique_value(
            &conn,
            "SELECT COUNT(*) FROM campaign_links WHERE slug = ?1",
            "dup_slug".to_string(),
            8,
            || {
                candidates
                    .next()
                    .unwrap_or_else(|| "fallback_slug".to_string())
            },
        )
        .expect("resolve unique");

        assert_eq!(resolved, "unique_slug");
    }
}
