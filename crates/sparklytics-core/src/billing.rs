use async_trait::async_trait;

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

/// Ingest admission gate.
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
}

/// Always allows; used by self-hosted mode.
pub struct NullBillingGate;

#[async_trait]
impl BillingGate for NullBillingGate {
    async fn admit_events(&self, _tenant_id: &str, requested_events: usize) -> BillingAdmission {
        BillingAdmission::allow_all(requested_events)
    }
}
