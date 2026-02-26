use chrono::{Duration, NaiveDate};
use serde_json::json;

use sparklytics_core::analytics::{
    resolve_comparison_range, CompareMode, ComparisonMetadata, ComparisonRange,
};

use crate::error::AppError;

fn parse_date(raw: &str, field: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest(format!("invalid {field} (expected YYYY-MM-DD)")))
}

fn parse_mode(raw: Option<&str>) -> Result<Option<CompareMode>, AppError> {
    CompareMode::parse(raw)
        .map(|mode| {
            if matches!(mode, CompareMode::None) {
                None
            } else {
                Some(mode)
            }
        })
        .map_err(|err| AppError::BadRequest(err.to_string()))
}

pub fn resolve_compare_range(
    primary_start: NaiveDate,
    primary_end: NaiveDate,
    mode_raw: Option<&str>,
    compare_start_raw: Option<&str>,
    compare_end_raw: Option<&str>,
) -> Result<Option<ComparisonRange>, AppError> {
    let Some(mode) = parse_mode(mode_raw)? else {
        return Ok(None);
    };

    let compare_start = compare_start_raw
        .map(|raw| parse_date(raw, "compare_start_date"))
        .transpose()?;
    let compare_end = compare_end_raw
        .map(|raw| parse_date(raw, "compare_end_date"))
        .transpose()?;

    resolve_comparison_range(primary_start, primary_end, mode, compare_start, compare_end)
        .map_err(|err| AppError::BadRequest(err.to_string()))
}

pub fn compare_metadata(compare: Option<&ComparisonRange>) -> Option<ComparisonMetadata> {
    compare.map(ComparisonRange::to_metadata)
}

pub fn default_previous_period_range(
    primary_start: NaiveDate,
    primary_end: NaiveDate,
) -> ComparisonRange {
    let primary_days = (primary_end - primary_start).num_days() + 1;
    let comparison_end = primary_start - Duration::days(1);
    let comparison_start = comparison_end - Duration::days(primary_days - 1);

    ComparisonRange {
        mode: CompareMode::PreviousPeriod,
        primary_start,
        primary_end,
        comparison_start,
        comparison_end,
    }
}

pub fn active_or_default_range(
    compare: Option<&ComparisonRange>,
    primary_start: NaiveDate,
    primary_end: NaiveDate,
) -> ComparisonRange {
    match compare {
        Some(range) => range.clone(),
        None => default_previous_period_range(primary_start, primary_end),
    }
}

pub fn mode_slug(mode: &CompareMode) -> &'static str {
    match mode {
        CompareMode::None => "none",
        CompareMode::PreviousPeriod => "previous_period",
        CompareMode::PreviousYear => "previous_year",
        CompareMode::Custom => "custom",
    }
}

pub fn metadata_json(compare: Option<&ComparisonRange>) -> Option<serde_json::Value> {
    compare.map(|range| {
        json!({
            "mode": mode_slug(&range.mode),
            "primary_range": [range.primary_start.to_string(), range.primary_end.to_string()],
            "comparison_range": [range.comparison_start.to_string(), range.comparison_end.to_string()],
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previous_period_shift_matches_primary_length() {
        let start = NaiveDate::from_ymd_opt(2026, 2, 10).expect("date");
        let end = NaiveDate::from_ymd_opt(2026, 2, 20).expect("date");
        let range = resolve_compare_range(start, end, Some("previous_period"), None, None)
            .expect("range")
            .expect("some");
        assert_eq!(
            range.comparison_end,
            NaiveDate::from_ymd_opt(2026, 2, 9).expect("date")
        );
        assert_eq!(
            range.comparison_start,
            NaiveDate::from_ymd_opt(2026, 1, 30).expect("date")
        );
    }

    #[test]
    fn custom_requires_dates() {
        let start = NaiveDate::from_ymd_opt(2026, 2, 10).expect("date");
        let end = NaiveDate::from_ymd_opt(2026, 2, 20).expect("date");
        let result = resolve_compare_range(start, end, Some("custom"), None, None);
        assert!(result.is_err());
    }

    #[test]
    fn custom_rejects_invalid_date() {
        let start = NaiveDate::from_ymd_opt(2026, 2, 10).expect("date");
        let end = NaiveDate::from_ymd_opt(2026, 2, 20).expect("date");
        let result =
            resolve_compare_range(start, end, Some("custom"), Some("bad"), Some("2026-01-01"));
        assert!(result.is_err());
    }

    #[test]
    fn oversized_custom_range_rejected() {
        let start = NaiveDate::from_ymd_opt(2026, 2, 10).expect("date");
        let end = NaiveDate::from_ymd_opt(2026, 2, 12).expect("date");
        let result = resolve_compare_range(
            start,
            end,
            Some("custom"),
            Some("2026-01-01"),
            Some("2026-01-31"),
        );
        assert!(result.is_err());
    }
}
