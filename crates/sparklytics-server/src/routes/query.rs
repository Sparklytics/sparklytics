use chrono::NaiveDate;

use crate::error::AppError;

pub(crate) fn parse_defaulted_date_range_lenient(
    start_date: Option<&str>,
    end_date: Option<&str>,
    default_lookback_days: i64,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    let today = chrono::Utc::now().date_naive();
    let start = start_date
        .and_then(parse_date_lenient)
        .unwrap_or_else(|| today - chrono::Duration::days(default_lookback_days));
    let end = end_date.and_then(parse_date_lenient).unwrap_or(today);
    validate_date_order(start, end)?;
    Ok((start, end))
}

pub(crate) fn parse_defaulted_date_range_strict(
    start_date: Option<&str>,
    end_date: Option<&str>,
    default_today: NaiveDate,
    default_lookback_days: i64,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    let start = start_date
        .map(|raw| parse_strict_date(raw, "start_date"))
        .transpose()?
        .unwrap_or_else(|| default_today - chrono::Duration::days(default_lookback_days));
    let end = end_date
        .map(|raw| parse_strict_date(raw, "end_date"))
        .transpose()?
        .unwrap_or(default_today);
    validate_date_order(start, end)?;
    Ok((start, end))
}

pub(crate) fn parse_required_date_range(
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    let Some(start_raw) = start_date else {
        return Err(AppError::BadRequest("start_date is required".to_string()));
    };
    let Some(end_raw) = end_date else {
        return Err(AppError::BadRequest("end_date is required".to_string()));
    };
    let start = parse_strict_date(start_raw, "start_date")?;
    let end = parse_strict_date(end_raw, "end_date")?;
    validate_date_order(start, end)?;
    Ok((start, end))
}

pub(crate) fn today_for_optional_timezone(timezone: Option<&str>) -> Result<NaiveDate, AppError> {
    match timezone {
        None => Ok(chrono::Utc::now().date_naive()),
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Ok(chrono::Utc::now().date_naive());
            }
            let tz = trimmed
                .parse::<chrono_tz::Tz>()
                .map_err(|_| AppError::BadRequest("invalid timezone".to_string()))?;
            Ok(chrono::Utc::now().with_timezone(&tz).date_naive())
        }
    }
}

pub(crate) fn normalize_timezone_non_empty(
    timezone: Option<&str>,
) -> Result<Option<String>, AppError> {
    match timezone {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(AppError::BadRequest(
                    "timezone cannot be empty when provided".to_string(),
                ));
            }
            Ok(Some(trimmed.to_string()))
        }
        None => Ok(None),
    }
}

pub(crate) fn normalize_optional_filter(
    field: &str,
    value: Option<String>,
    max_len: usize,
) -> Result<Option<String>, AppError> {
    if let Some(raw) = value {
        let trimmed = raw.trim().to_string();
        if trimmed.is_empty() {
            return Err(AppError::BadRequest(format!(
                "{field} cannot be empty when provided"
            )));
        }
        if trimmed.len() > max_len {
            return Err(AppError::BadRequest(format!(
                "{field} is too long (max {max_len} characters)"
            )));
        }
        return Ok(Some(trimmed));
    }
    Ok(None)
}

pub(crate) fn parse_optional_bool(
    value: Option<&str>,
    field: &str,
) -> Result<Option<bool>, AppError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(Some(true)),
        "false" | "0" => Ok(Some(false)),
        _ => Err(AppError::BadRequest(format!(
            "{field} must be one of: true, false, 1, 0"
        ))),
    }
}

pub(crate) fn validate_date_span(
    start: NaiveDate,
    end: NaiveDate,
    max_days: i64,
    field_name: &str,
) -> Result<(), AppError> {
    let range_days = (end - start).num_days() + 1;
    if range_days > max_days {
        return Err(AppError::BadRequest(format!(
            "{field_name} too large: {range_days} days (max {max_days})"
        )));
    }
    Ok(())
}

fn parse_date_lenient(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").ok()
}

fn parse_strict_date(raw: &str, field: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest(format!("invalid {field} (expected YYYY-MM-DD)")))
}

fn validate_date_order(start: NaiveDate, end: NaiveDate) -> Result<(), AppError> {
    if end < start {
        return Err(AppError::BadRequest(
            "end_date must be on or after start_date".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{
        parse_defaulted_date_range_lenient, parse_optional_bool, parse_required_date_range,
        validate_date_span,
    };

    #[test]
    fn parse_optional_bool_accepts_common_variants() {
        assert_eq!(
            parse_optional_bool(Some("true"), "include_bots").expect("bool"),
            Some(true)
        );
        assert_eq!(
            parse_optional_bool(Some("0"), "include_bots").expect("bool"),
            Some(false)
        );
        assert_eq!(
            parse_optional_bool(None, "include_bots").expect("bool"),
            None
        );
    }

    #[test]
    fn parse_optional_bool_rejects_invalid_values() {
        assert!(parse_optional_bool(Some("yes"), "include_bots").is_err());
    }

    #[test]
    fn parse_required_date_range_rejects_reversed_bounds() {
        let result = parse_required_date_range(Some("2026-01-05"), Some("2026-01-01"));
        assert!(result.is_err());
    }

    #[test]
    fn validate_date_span_rejects_large_ranges() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid date");
        let end = NaiveDate::from_ymd_opt(2026, 4, 15).expect("valid date");
        assert!(validate_date_span(start, end, 90, "date range").is_err());
    }

    #[test]
    fn parse_defaulted_lenient_rejects_reversed_bounds() {
        let result = parse_defaulted_date_range_lenient(Some("2026-01-05"), Some("2026-01-01"), 6);
        assert!(result.is_err());
    }

    #[test]
    fn parse_defaulted_lenient_ignores_invalid_date_format() {
        let result = parse_defaulted_date_range_lenient(Some("bad-date"), None, 6);
        assert!(result.is_ok());
    }
}
