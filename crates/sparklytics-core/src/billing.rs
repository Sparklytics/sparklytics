use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BillingLimitReason {
    MonthlyLimit,
    PeakRate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingAdmission {
    pub allowed_events: usize,
    pub reason: Option<BillingLimitReason>,
}

impl BillingAdmission {
    pub fn allow_all(requested_events: usize) -> Self {
        Self {
            allowed_events: requested_events,
            reason: None,
        }
    }

    pub fn limited(allowed_events: usize, reason: BillingLimitReason) -> Self {
        Self {
            allowed_events,
            reason: Some(reason),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanLimitConfig {
    pub plan: String,
    pub peak_events_per_sec: u32,
    pub monthly_event_limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantLimitOverride {
    pub tenant_id: String,
    pub peak_events_per_sec: Option<u32>,
    pub monthly_event_limit: Option<u64>,
    pub updated_by: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantEffectiveLimits {
    pub tenant_id: String,
    pub plan: String,
    pub peak_events_per_sec: u32,
    pub monthly_event_limit: u64,
    pub override_peak_events_per_sec: Option<u32>,
    pub override_monthly_event_limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantMonthlyUsage {
    pub tenant_id: String,
    pub month: String,
    pub event_count: u64,
    pub event_limit: u64,
    pub percent_used: f64,
}

/// Plan-limit gate.
#[async_trait]
pub trait BillingGate: Send + Sync + 'static {
    async fn admit_events(&self, tenant_id: &str, requested_events: usize) -> BillingAdmission;
    async fn record_persisted_events(
        &self,
        _tenant_id: &str,
        _persisted_events: usize,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn list_plan_limits(&self) -> anyhow::Result<Vec<PlanLimitConfig>> {
        anyhow::bail!("plan limit administration is not available in this runtime");
    }

    async fn upsert_plan_limit(
        &self,
        _plan: &str,
        _peak_events_per_sec: u32,
        _monthly_event_limit: u64,
    ) -> anyhow::Result<PlanLimitConfig> {
        anyhow::bail!("plan limit administration is not available in this runtime");
    }

    async fn get_tenant_override(
        &self,
        _tenant_id: &str,
    ) -> anyhow::Result<Option<TenantLimitOverride>> {
        anyhow::bail!("tenant override administration is not available in this runtime");
    }

    async fn upsert_tenant_override(
        &self,
        _tenant_id: &str,
        _peak_events_per_sec: Option<u32>,
        _monthly_event_limit: Option<u64>,
        _updated_by: &str,
    ) -> anyhow::Result<TenantLimitOverride> {
        anyhow::bail!("tenant override administration is not available in this runtime");
    }

    async fn clear_tenant_override(
        &self,
        _tenant_id: &str,
        _updated_by: &str,
    ) -> anyhow::Result<()> {
        anyhow::bail!("tenant override administration is not available in this runtime");
    }

    async fn get_tenant_effective_limits(
        &self,
        _tenant_id: &str,
    ) -> anyhow::Result<TenantEffectiveLimits> {
        anyhow::bail!("tenant effective limits are not available in this runtime");
    }

    async fn get_tenant_monthly_usage(
        &self,
        _tenant_id: &str,
        _month: Option<&str>,
    ) -> anyhow::Result<TenantMonthlyUsage> {
        anyhow::bail!("tenant usage is not available in this runtime");
    }
}

/// Always allows; used by self-hosted mode.
pub struct NullBillingGate;

#[async_trait]
impl BillingGate for NullBillingGate {
    async fn admit_events(&self, _tenant_id: &str, requested_events: usize) -> BillingAdmission {
        BillingAdmission::allow_all(requested_events)
    }
}
