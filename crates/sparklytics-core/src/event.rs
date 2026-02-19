use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The payload the client sends to POST /api/collect.
/// Wire field "type" maps to event_type in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectPayload {
    pub website_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub url: String,
    pub referrer: Option<String>,
    /// Combined screen resolution string, e.g. "1920x1080".
    pub screen: Option<String>,
    /// Screen width in pixels (alternative to the combined `screen` string).
    /// If both `screen_width` and `screen_height` are provided and `screen` is
    /// absent, the server combines them as "{width}x{height}".
    pub screen_width: Option<u32>,
    pub screen_height: Option<u32>,
    pub language: Option<String>,
    pub event_name: Option<String>,
    /// Client sends a JSON object; server serializes to String before DuckDB storage.
    pub event_data: Option<serde_json::Value>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_term: Option<String>,
    pub utm_content: Option<String>,
}

/// Accepts either a single event or a batch array at POST /api/collect.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum CollectOrBatch {
    Single(Box<CollectPayload>),
    Batch(Vec<CollectPayload>),
}

/// The enriched, stored version of an event — mirrors the DuckDB `events` table columns exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub website_id: String,
    /// NULL in self-hosted mode; set to Clerk org_id in cloud mode.
    pub tenant_id: Option<String>,
    pub session_id: String,
    pub visitor_id: String,
    pub event_type: String,
    pub url: String,
    pub referrer_url: Option<String>,
    pub referrer_domain: Option<String>,
    pub event_name: Option<String>,
    /// Serialized JSON string. Client sends an object; server stringifies before storage.
    pub event_data: Option<String>,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub browser: Option<String>,
    pub browser_version: Option<String>,
    pub os: Option<String>,
    pub os_version: Option<String>,
    pub device_type: Option<String>,
    pub screen: Option<String>,
    pub language: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_term: Option<String>,
    pub utm_content: Option<String>,
    pub created_at: DateTime<Utc>,
}
