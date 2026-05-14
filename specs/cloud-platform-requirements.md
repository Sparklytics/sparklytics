# Private Cloud Platform Boundary

**Status:** Private-repo owned
**Last Updated:** 2026-05-11

The public `sparklytics/sparklytics` repository ships the MIT-licensed
self-hosted runtime: Rust/Axum/Tokio, DuckDB local storage, embedded dashboard,
local auth, local API keys, `/s.js`, `/api/collect`, and `/e`.

Hosted-cloud platform requirements are intentionally not specified here. The
private cloud repo owns:

- hosted identity/auth integration
- tenant control-plane data
- billing and subscription enforcement
- payment-provider integration
- warehouse/runtime backends beyond public DuckDB self-host
- cloud migrations and private ops configuration
- hosted-cloud deployment topology and monitoring runbooks

The only public integration boundary is the minimal runtime trait surface needed
by the self-hosted binary:

- `BillingGate`
- `NullBillingGate`
- shared event and analytics domain types

Public self-hosted builds must not depend on private cloud secrets, billing
providers, cloud auth providers, hosted warehouse credentials, private ops
files, or cloud migrations.
