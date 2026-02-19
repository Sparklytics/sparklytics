use std::future::Future;
use std::pin::Pin;

use super::{BillingGate, BillingOutcome};

/// [`BillingGate`] that always allows all requests.
///
/// Used in self-hosted mode and in the public cloud binary (the private
/// `sparklytics-billing` crate injects `StripeBillingGate` for paid plans).
pub struct NullBillingGate;

impl BillingGate for NullBillingGate {
    fn check<'a>(
        &'a self,
        _tenant_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = BillingOutcome> + Send + 'a>> {
        Box::pin(async { BillingOutcome::Allowed })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BDD: NullBillingGate always allows (self-hosted default).
    #[tokio::test]
    async fn test_null_billing_gate_always_allows() {
        let gate = NullBillingGate;
        assert_eq!(gate.check("org_any").await, BillingOutcome::Allowed);
        assert_eq!(gate.check("org_abc123").await, BillingOutcome::Allowed);
        assert_eq!(gate.check("").await, BillingOutcome::Allowed);
    }
}
