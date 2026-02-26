use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use lettre::{
    message::Mailbox, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use serde_json::Value;
use sparklytics_core::analytics::{
    NotificationChannel, NotificationDelivery, NotificationDeliveryStatus, NotificationSourceType,
};
use tracing::{info, warn};

use crate::state::AppState;

fn is_valid_email(target: &str) -> bool {
    let trimmed = target.trim();
    let Some((local, domain)) = trimmed.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.')
}

async fn deliver_email(target: String, payload: Value) -> Result<(), String> {
    if !is_valid_email(&target) {
        return Err("invalid email target".to_string());
    }
    let smtp_noop_enabled = std::env::var("SPARKLYTICS_SMTP_NOOP")
        .ok()
        .map(|v| {
            let trimmed = v.trim();
            trimmed.eq_ignore_ascii_case("1")
                || trimmed.eq_ignore_ascii_case("true")
                || trimmed.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false);
    if smtp_noop_enabled {
        info!(
            target = %target,
            "SMTP noop transport enabled; marking delivery as sent without network dispatch"
        );
        return Ok(());
    }
    let host = std::env::var("SPARKLYTICS_SMTP_HOST")
        .map_err(|_| "smtp host is not configured".to_string())?;
    let port = std::env::var("SPARKLYTICS_SMTP_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(587);
    let from_value = std::env::var("SPARKLYTICS_SMTP_FROM")
        .unwrap_or_else(|_| "sparklytics@localhost".to_string());
    let from: Mailbox = from_value
        .parse()
        .map_err(|_| "invalid SPARKLYTICS_SMTP_FROM".to_string())?;
    let to: Mailbox = target
        .parse()
        .map_err(|_| "invalid email target".to_string())?;
    let email = Message::builder()
        .from(from)
        .to(to)
        .subject("Sparklytics Notification")
        .body(payload.to_string())
        .map_err(|e| format!("smtp message build failed: {e}"))?;

    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
        .port(port)
        .timeout(Some(Duration::from_secs(5)));
    if let (Ok(user), Ok(pass)) = (
        std::env::var("SPARKLYTICS_SMTP_USERNAME"),
        std::env::var("SPARKLYTICS_SMTP_PASSWORD"),
    ) {
        builder = builder.credentials(Credentials::new(user, pass));
    }
    let mailer = builder.build();
    mailer
        .send(email)
        .await
        .map_err(|e| format!("smtp send failed: {e}"))?;
    Ok(())
}

async fn deliver_webhook(target: String, payload: Value) -> Result<(), String> {
    let parsed = url::Url::parse(target.trim()).map_err(|_| "invalid webhook url".to_string())?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err("webhook url must use http or https".to_string());
    }
    let target_for_req = target.clone();
    let payload_text = payload.to_string();
    let host = parsed
        .host_str()
        .ok_or_else(|| "webhook url missing host".to_string())?;
    if host.eq_ignore_ascii_case("localhost") {
        return Err("webhook target host is not allowed".to_string());
    }
    let host_owned = host.to_string();
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| "webhook url missing port".to_string())?;
    let host_for_dns = host_owned.clone();
    let resolved: Vec<SocketAddr> = tokio::task::spawn_blocking(move || {
        (host_for_dns.as_str(), port)
            .to_socket_addrs()
            .map(|iter| iter.collect::<Vec<_>>())
    })
    .await
    .map_err(|e| format!("webhook dns task join failed: {e}"))?
    .map_err(|e| format!("webhook dns resolve failed: {e}"))?;
    if resolved.is_empty() {
        return Err("webhook dns resolve returned no addresses".to_string());
    }
    if resolved.iter().any(|addr| is_disallowed_ip(addr.ip())) {
        return Err("webhook target resolves to non-public address".to_string());
    }

    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none());
    if host_owned.parse::<IpAddr>().is_err() {
        for addr in &resolved {
            builder = builder.resolve(host_owned.as_str(), *addr);
        }
    }
    let client = builder
        .build()
        .map_err(|e| format!("webhook client build failed: {e}"))?;
    let response = client
        .post(&target_for_req)
        .header("content-type", "application/json")
        .body(payload_text)
        .send()
        .await
        .map_err(|e| format!("webhook send failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "webhook responded with status {}",
            response.status()
        ));
    }
    Ok(())
}

fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.octets()[0] == 0
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6.is_multicast()
                || v6
                    .to_ipv4()
                    .map(|v4| {
                        v4.is_private()
                            || v4.is_loopback()
                            || v4.is_link_local()
                            || v4.is_multicast()
                            || v4.is_broadcast()
                            || v4.is_unspecified()
                            || v4.octets()[0] == 0
                    })
                    .unwrap_or(false)
        }
    }
}

pub async fn deliver_and_record(
    state: &Arc<AppState>,
    source_type: NotificationSourceType,
    source_id: &str,
    idempotency_key: &str,
    channel: NotificationChannel,
    target: String,
    payload: Value,
) -> anyhow::Result<Option<NotificationDelivery>> {
    if state
        .scheduler_db
        .has_notification_delivery(idempotency_key)
        .await?
    {
        return Ok(None);
    }

    let result = match channel {
        NotificationChannel::Email => deliver_email(target, payload).await,
        NotificationChannel::Webhook => deliver_webhook(target, payload).await,
    };

    let (status, error_message) = match result {
        Ok(()) => (NotificationDeliveryStatus::Sent, None),
        Err(err) => {
            warn!(
                source_type = ?source_type,
                source_id = source_id,
                error = %err,
                "notification delivery failed"
            );
            (NotificationDeliveryStatus::Failed, Some(err))
        }
    };

    let delivery = state
        .scheduler_db
        .create_notification_delivery(
            source_type,
            source_id,
            idempotency_key,
            status,
            error_message.as_deref(),
        )
        .await?;
    Ok(Some(delivery))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sparklytics_core::config::{AuthMode, Config};
    use sparklytics_duckdb::DuckDbBackend;

    use crate::state::AppState;

    use super::*;

    fn unique_data_dir() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("unix time")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("sparklytics-notify-delivery-{nanos}"))
            .to_string_lossy()
            .to_string()
    }

    #[tokio::test]
    async fn idempotency_key_prevents_duplicate_delivery_rows() {
        let data_dir = unique_data_dir();
        std::fs::create_dir_all(&data_dir).expect("create temp dir");
        let db_path = format!("{data_dir}/sparklytics.db");
        let db = DuckDbBackend::open(&db_path, "1GB").expect("open db");
        let state = Arc::new(AppState::new(
            db,
            Config {
                port: 0,
                data_dir,
                geoip_path: "./GeoLite2-City.mmdb".to_string(),
                auth_mode: AuthMode::None,
                https: false,
                retention_days: 365,
                cors_origins: vec![],
                session_days: 7,
                buffer_flush_interval_ms: 5_000,
                buffer_max_size: 100,
                mode: sparklytics_core::config::AppMode::SelfHosted,
                argon2_memory_kb: 65_536,
                public_url: "http://localhost:3000".to_string(),
                rate_limit_disable: false,
                duckdb_memory_limit: "1GB".to_string(),
            },
        ));

        let key = "idem:test-key";
        let first = deliver_and_record(
            &state,
            NotificationSourceType::Alert,
            "alert_test",
            key,
            NotificationChannel::Email,
            "team@example.com".to_string(),
            serde_json::json!({"hello": "world"}),
        )
        .await
        .expect("first delivery");
        assert!(first.is_some(), "first delivery should be recorded");

        let second = deliver_and_record(
            &state,
            NotificationSourceType::Alert,
            "alert_test",
            key,
            NotificationChannel::Email,
            "team@example.com".to_string(),
            serde_json::json!({"hello": "world"}),
        )
        .await
        .expect("second delivery");
        assert!(second.is_none(), "second delivery should be skipped");
    }
}
