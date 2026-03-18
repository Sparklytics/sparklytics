use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub data_dir: String,
    pub geoip_path: String,
    pub auth_mode: AuthMode,
    pub bootstrap_password: Option<String>,
    pub https: bool,
    pub retention_days: u32,
    pub cors_origins: Vec<String>,
    pub session_days: u32,
    pub buffer_flush_interval_ms: u64,
    pub buffer_max_size: usize,
    pub mode: AppMode,
    pub argon2_memory_kb: u32,
    pub public_url: String,
    /// When true, skip the rate limiter on /api/collect. For benchmarking only.
    /// Controlled by SPARKLYTICS_RATE_LIMIT_DISABLE=true. Never document in README.
    pub rate_limit_disable: bool,
    /// DuckDB memory limit passed to `SET memory_limit = '...'` at init.
    /// Accepts any DuckDB size string: `"512MB"`, `"1GB"`, `"4GB"`, etc.
    /// Controlled by `SPARKLYTICS_DUCKDB_MEMORY` (default `"1GB"`).
    /// Modern 4–32 GB VPS instances can set 2–8 GB for better query performance.
    pub duckdb_memory_limit: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuthMode {
    None,
    /// Holds the plaintext password value read from `SPARKLYTICS_PASSWORD`.
    Password(String),
    Local,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    SelfHosted,
    Cloud,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        Self::from_env_with(|key| std::env::var(key).ok())
    }

    fn from_env_with<F>(get_var: F) -> Result<Self, String>
    where
        F: Fn(&str) -> Option<String>,
    {
        let public_url =
            get_var("SPARKLYTICS_PUBLIC_URL").unwrap_or_else(|| "http://localhost:3000".to_string());
        let public_url = public_url.trim().trim_end_matches('/').to_string();
        if public_url.is_empty() {
            return Err("SPARKLYTICS_PUBLIC_URL must not be empty".to_string());
        }

        Ok(Self {
            port: get_var("SPARKLYTICS_PORT")
                .unwrap_or_else(|| "3000".to_string())
                .parse()
                .map_err(|e| format!("invalid port: {e}"))?,
            data_dir: get_var("SPARKLYTICS_DATA_DIR")
                .unwrap_or_else(|| "./data".to_string()),
            geoip_path: get_var("SPARKLYTICS_GEOIP_PATH")
                .unwrap_or_else(|| "./GeoLite2-City.mmdb".to_string()),
            auth_mode: {
                let raw = get_var("SPARKLYTICS_AUTH").unwrap_or_else(|| "local".to_string());
                match raw.as_str() {
                    "none" => AuthMode::None,
                    "password" => {
                        let pw = get_var("SPARKLYTICS_PASSWORD").ok_or_else(|| {
                            "SPARKLYTICS_PASSWORD required when AUTH=password".to_string()
                        })?;
                        AuthMode::Password(pw)
                    }
                    _ => AuthMode::Local,
                }
            },
            bootstrap_password: get_var("SPARKLYTICS_BOOTSTRAP_PASSWORD"),
            https: get_var("SPARKLYTICS_HTTPS")
                .map(|v| v == "true")
                .unwrap_or(true),
            retention_days: get_var("SPARKLYTICS_RETENTION_DAYS")
                .unwrap_or_else(|| "365".to_string())
                .parse()
                .unwrap_or(365),
            cors_origins: get_var("SPARKLYTICS_CORS_ORIGINS")
                .map(|v| v.split(',').map(str::to_string).collect())
                .unwrap_or_default(),
            session_days: get_var("SPARKLYTICS_SESSION_DAYS")
                .unwrap_or_else(|| "7".to_string())
                .parse()
                .unwrap_or(7),
            buffer_flush_interval_ms: 5000, // Sprint-0 spec: flush every 5s
            buffer_max_size: 100,           // Sprint-0 spec: flush immediately at 100 events
            mode: {
                let raw = get_var("SPARKLYTICS_MODE").unwrap_or_else(|| "selfhosted".to_string());
                match raw.as_str() {
                    "cloud" => AppMode::Cloud,
                    _ => AppMode::SelfHosted,
                }
            },
            argon2_memory_kb: get_var("SPARKLYTICS_ARGON2_MEMORY_KB")
                .unwrap_or_else(|| "65536".to_string())
                .parse()
                .unwrap_or(65536),
            public_url,
            rate_limit_disable: get_var("SPARKLYTICS_RATE_LIMIT_DISABLE")
                .map(|v| v == "true")
                .unwrap_or(false),
            duckdb_memory_limit: get_var("SPARKLYTICS_DUCKDB_MEMORY")
                .unwrap_or_else(|| "1GB".to_string()),
        })
    }

    pub fn buffer_flush_interval(&self) -> Duration {
        Duration::from_millis(self.buffer_flush_interval_ms)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::Config;

    #[test]
    fn trims_trailing_slash_from_public_url() {
        let vars = HashMap::from([(
            "SPARKLYTICS_PUBLIC_URL",
            "https://analytics.example.com/".to_string(),
        )]);

        let cfg = Config::from_env_with(|key| vars.get(key).cloned()).expect("config");

        assert_eq!(cfg.public_url, "https://analytics.example.com");
    }

    #[test]
    fn rejects_empty_public_url() {
        let vars = HashMap::from([("SPARKLYTICS_PUBLIC_URL", "   ".to_string())]);

        let err = Config::from_env_with(|key| vars.get(key).cloned()).expect_err("empty url");

        assert_eq!(err, "SPARKLYTICS_PUBLIC_URL must not be empty");
    }
}
