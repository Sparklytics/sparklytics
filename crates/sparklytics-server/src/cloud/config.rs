/// Configuration for cloud mode (SPARKLYTICS_MODE=cloud).
///
/// Loaded once at startup. If any required variable is missing the process
/// panics immediately with a clear human-readable message before binding any
/// port — this is the "fail-fast on misconfiguration" requirement from the
/// Sprint 5 BDD scenarios.
#[derive(Debug, Clone)]
pub struct CloudConfig {
    pub clerk_secret_key: String,
    pub clerk_webhook_signing_secret: String,
    pub database_url: String,
    pub clickhouse_url: String,
    pub clickhouse_user: String,
    pub clickhouse_password: String,
}

impl CloudConfig {
    /// Read cloud configuration from environment variables.
    ///
    /// # Panics
    /// Panics immediately if any required variable is missing, printing a clear
    /// message naming the variable (e.g., "CLERK_SECRET_KEY is required in cloud mode").
    pub fn from_env() -> Self {
        let require = |name: &'static str| -> String {
            std::env::var(name).unwrap_or_else(|_| panic!("{name} is required in cloud mode"))
        };

        Self {
            clerk_secret_key: require("CLERK_SECRET_KEY"),
            clerk_webhook_signing_secret: require("CLERK_WEBHOOK_SIGNING_SECRET"),
            database_url: require("DATABASE_URL"),
            clickhouse_url: require("CLICKHOUSE_URL"),
            clickhouse_user: require("CLICKHOUSE_USER"),
            clickhouse_password: require("CLICKHOUSE_PASSWORD"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// Serializes all env-var mutation tests in this module.
    ///
    /// Both tests mutate process-global env vars and then trigger a panic via
    /// `#[should_panic]`. Because Rust runs tests in parallel by default, two
    /// env-mutating tests can interleave and produce wrong panic messages.
    ///
    /// Using `unwrap_or_else(|p| p.into_inner())` recovers from lock poisoning
    /// caused by the expected panics, so each subsequent test gets a clean lock.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// BDD: Missing CLERK_SECRET_KEY panics with the required message.
    #[test]
    #[should_panic(expected = "CLERK_SECRET_KEY is required in cloud mode")]
    fn test_missing_clerk_secret_panics() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        for var in &[
            "CLERK_SECRET_KEY",
            "CLERK_WEBHOOK_SIGNING_SECRET",
            "DATABASE_URL",
            "CLICKHOUSE_URL",
            "CLICKHOUSE_USER",
            "CLICKHOUSE_PASSWORD",
        ] {
            unsafe { std::env::remove_var(var) };
        }
        CloudConfig::from_env();
    }

    /// BDD: Missing DATABASE_URL panics with the required message.
    #[test]
    #[should_panic(expected = "DATABASE_URL is required in cloud mode")]
    fn test_missing_database_url_panics() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        unsafe {
            std::env::set_var("CLERK_SECRET_KEY", "sk_test");
            std::env::set_var("CLERK_WEBHOOK_SIGNING_SECRET", "whsec_test");
            std::env::remove_var("DATABASE_URL");
            std::env::set_var("CLICKHOUSE_URL", "http://localhost:8123");
            std::env::set_var("CLICKHOUSE_USER", "ch");
            std::env::set_var("CLICKHOUSE_PASSWORD", "pw");
        }
        CloudConfig::from_env();
    }
}
