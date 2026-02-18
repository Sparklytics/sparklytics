use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub data_dir: String,
    pub geoip_path: String,
    pub auth_mode: AuthMode,
    pub https: bool,
    pub retention_days: u32,
    pub cors_origins: Vec<String>,
    pub session_days: u32,
    pub buffer_flush_interval_ms: u64,
    pub buffer_max_size: usize,
    pub mode: AppMode,
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
        Ok(Self {
            port: std::env::var("SPARKLYTICS_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .map_err(|e| format!("invalid port: {e}"))?,
            data_dir: std::env::var("SPARKLYTICS_DATA_DIR")
                .unwrap_or_else(|_| "./data".to_string()),
            geoip_path: std::env::var("SPARKLYTICS_GEOIP_PATH")
                .unwrap_or_else(|_| "./GeoLite2-City.mmdb".to_string()),
            auth_mode: {
                let raw = std::env::var("SPARKLYTICS_AUTH")
                    .unwrap_or_else(|_| "local".to_string());
                match raw.as_str() {
                    "none" => AuthMode::None,
                    "password" => {
                        let pw = std::env::var("SPARKLYTICS_PASSWORD").map_err(|_| {
                            "SPARKLYTICS_PASSWORD required when AUTH=password".to_string()
                        })?;
                        AuthMode::Password(pw)
                    }
                    _ => AuthMode::Local,
                }
            },
            https: std::env::var("SPARKLYTICS_HTTPS")
                .map(|v| v == "true")
                .unwrap_or(true),
            retention_days: std::env::var("SPARKLYTICS_RETENTION_DAYS")
                .unwrap_or_else(|_| "365".to_string())
                .parse()
                .unwrap_or(365),
            cors_origins: std::env::var("SPARKLYTICS_CORS_ORIGINS")
                .map(|v| v.split(',').map(str::to_string).collect())
                .unwrap_or_default(),
            session_days: std::env::var("SPARKLYTICS_SESSION_DAYS")
                .unwrap_or_else(|_| "7".to_string())
                .parse()
                .unwrap_or(7),
            buffer_flush_interval_ms: 1000,
            buffer_max_size: 1000,
            mode: {
                let raw = std::env::var("SPARKLYTICS_MODE")
                    .unwrap_or_else(|_| "selfhosted".to_string());
                match raw.as_str() {
                    "cloud" => AppMode::Cloud,
                    _ => AppMode::SelfHosted,
                }
            },
        })
    }

    pub fn buffer_flush_interval(&self) -> Duration {
        Duration::from_millis(self.buffer_flush_interval_ms)
    }
}
