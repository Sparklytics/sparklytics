# Tasks

_Last updated: 2026-05-11_

## In Progress

- Cut and validate a release candidate before public launch
- Resolve GHCR package visibility for the optional prebuilt-image install path

## Up Next

- Keep running the full release matrix before any release candidate:
- `cargo fmt --check`
- `cargo check`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo build --release`
- `node scripts/selfhost-api-smoke.mjs`
- `cd dashboard && npm run lint`
- `cd dashboard && npm run type-check`
- `cd dashboard && npm run build`
- `cd dashboard && npm run test:e2e -- first-launch-onboarding.spec.ts`
- `cd dashboard && npm run test:release-smoke`
- `docker compose config`
- `docker compose -f docker-compose.caddy.yml config`
- `docker compose -f docker-compose.image.yml config`
- `cd dashboard && npm audit --omit=dev --audit-level=high`

## Backlog

## Done

- Zero-websites first-launch flow
- Setup -> login -> onboarding handoff
- First website creation and snippet verification guidance
- Release readiness gate for source-build self-host path
- Docs and release drift cleanup for first-launch source-build flow
- Fresh-install API smoke command (`node scripts/selfhost-api-smoke.mjs`)
- Deterministic dashboard release smoke (`npm run test:release-smoke`)
- Production dependency audit is clean after replacing `react-simple-maps` with the local D3 renderer
