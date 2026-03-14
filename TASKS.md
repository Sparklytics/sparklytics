# Tasks

_Last updated: 2026-03-08_

## In Progress

- Release readiness gate for first public self-hosted launch
- Docs and release drift cleanup
- Fresh-install smoke command and deterministic dashboard verification

## Up Next

- Run the full release matrix:
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo build --release`
- `cd dashboard && npm run build`
- Selected Playwright suite including explicit first-launch smoke (`npm run test:e2e -- first-launch-onboarding.spec.ts` and `npm run test:release-smoke`)
- `docker compose config`
- `docker compose -f docker-compose.caddy.yml config`
- Perform one Docker-first cold install from docs only
- Cut and validate a release candidate before public launch

## Backlog

## Done

- Zero-websites first-launch flow
- Setup -> login -> onboarding handoff
- First website creation and snippet verification guidance
