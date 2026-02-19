use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BillingOutcome {
    Allowed,
    LimitExceeded,
}

/// Plan-limit gate.
#[async_trait]
pub trait BillingGate: Send + Sync + 'static {
    async fn check(&self, tenant_id: &str) -> BillingOutcome;
}

/// Always allows; used by self-hosted mode.
pub struct NullBillingGate;

#[async_trait]
impl BillingGate for NullBillingGate {
    async fn check(&self, _tenant_id: &str) -> BillingOutcome {
        BillingOutcome::Allowed
    }
}
