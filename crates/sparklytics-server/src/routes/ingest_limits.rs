use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{de::Error as _, Deserialize, Deserializer};
use serde_json::json;

use sparklytics_metadata::UpdateWebsiteParams;

use crate::{error::AppError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct UpdateIngestLimitsRequest {
    #[serde(default, deserialize_with = "deserialize_tri_state")]
    pub peak_events_per_sec: Option<Option<i64>>,
    #[serde(default, deserialize_with = "deserialize_tri_state")]
    pub queue_max_events: Option<Option<i64>>,
}

fn deserialize_tri_state<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None => Ok(Some(None)),
        Some(raw) => T::deserialize(raw)
            .map(|parsed| Some(Some(parsed)))
            .map_err(D::Error::custom),
    }
}

fn clamp_positive_i64(value: Option<i64>, field: &str) -> Result<Option<i64>, AppError> {
    let Some(v) = value else {
        return Ok(None);
    };
    if v <= 0 {
        return Err(AppError::BadRequest(format!("{field} must be > 0")));
    }
    Ok(Some(v))
}

fn build_limits_payload(
    state: &AppState,
    website_id: &str,
    website: &sparklytics_metadata::Website,
) -> serde_json::Value {
    let peak_custom = website
        .ingest_peak_eps
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0);
    let queue_custom = website
        .ingest_queue_max_events
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0);

    let peak_effective = peak_custom.unwrap_or(state.website_ingest_peak_eps_default());
    let queue_effective = queue_custom.unwrap_or(state.website_ingest_queue_max_events_default());

    json!({
        "website_id": website_id,
        "peak_events_per_sec": peak_effective,
        "queue_max_events": queue_effective,
        "custom": {
            "peak_events_per_sec": peak_custom,
            "queue_max_events": queue_custom,
        },
        "source": {
            "peak_events_per_sec": if peak_custom.is_some() { "custom" } else { "default" },
            "queue_max_events": if queue_custom.is_some() { "custom" } else { "default" },
        }
    })
}

pub async fn get_ingest_limits(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let website = state
        .get_website_metadata_cached(&website_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("Website not found".to_string()))?;

    Ok(Json(json!({
        "data": build_limits_payload(&state, &website_id, &website),
    })))
}

pub async fn update_ingest_limits(
    State(state): State<Arc<AppState>>,
    Path(website_id): Path<String>,
    Json(body): Json<UpdateIngestLimitsRequest>,
) -> Result<impl IntoResponse, AppError> {
    if body.peak_events_per_sec.is_none() && body.queue_max_events.is_none() {
        return Err(AppError::BadRequest(
            "at least one ingest limit field must be provided".to_string(),
        ));
    }

    let peak_events_per_sec = match body.peak_events_per_sec {
        Some(value) => Some(clamp_positive_i64(value, "peak_events_per_sec")?),
        None => None,
    };
    let queue_max_events = match body.queue_max_events {
        Some(value) => Some(clamp_positive_i64(value, "queue_max_events")?),
        None => None,
    };

    let website = state
        .metadata
        .update_website(
            &website_id,
            UpdateWebsiteParams {
                name: None,
                domain: None,
                timezone: None,
                ingest_peak_eps: peak_events_per_sec,
                ingest_queue_max_events: queue_max_events,
            },
        )
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("Website not found".to_string()))?;

    state.cache_website_metadata(website.clone()).await;

    Ok(Json(json!({
        "data": build_limits_payload(&state, &website_id, &website),
    })))
}
