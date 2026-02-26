/// Append a bot filter predicate for event aliases when `include_bots=false`.
pub fn append_event_bot_filter(filter_sql: &mut String, include_bots: bool, column_prefix: &str) {
    if !include_bots {
        filter_sql.push_str(&format!(" AND {column_prefix}is_bot = FALSE"));
    }
}

/// Append a bot filter predicate for session aliases when `include_bots=false`.
pub fn append_session_bot_filter(filter_sql: &mut String, include_bots: bool, column_prefix: &str) {
    if !include_bots {
        filter_sql.push_str(&format!(" AND {column_prefix}is_bot = FALSE"));
    }
}
