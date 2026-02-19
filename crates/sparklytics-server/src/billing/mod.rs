use std::future::Future;
use std::pin::Pin;

pub mod null;

pub use null::NullBillingGate;

/// Outcome of a [`BillingGate::check`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BillingOutcome {
    Allowed,
    LimitExceeded,
}

/// Trait for plan-limit enforcement.
///
/// Self-hosted mode always uses [`NullBillingGate`] (always allows).
/// Cloud mode will inject `StripeBillingGate` from the private
/// `sparklytics-billing` crate (see CLAUDE.md critical fact #13).
///
/// The trait is object-safe via an explicit boxed-future return type so it can
/// be stored as `Arc<dyn BillingGate>` in [`AppState`].
pub trait BillingGate: Send + Sync {
    fn check<'a>(
        &'a self,
        tenant_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = BillingOutcome> + Send + 'a>>;
}
