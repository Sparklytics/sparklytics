# Contributing to Sparklytics

Thank you for your interest in contributing. This document covers setup, workflow, and code style.

---

## Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | stable | Backend |
| Node.js | ≥ 20 | Dashboard + SDK |
| Docker | ≥ 24 | Container builds |
| `cargo-audit` | latest | Security audit |

Install Rust: https://rustup.rs

---

## Local Setup

```bash
git clone https://github.com/sparklytics/sparklytics.git
cd sparklytics

# Backend
cargo build
cargo test

# Dashboard checks
cd dashboard
npm ci
npm run lint
npm run type-check
npm run build

# Optional SDK work (separate nested repo checkout)
cd ../sdk/next
npm ci
npm run build
npm test
```

---

## Development Workflow

1. Fork the repo and create a branch: `git checkout -b feat/my-feature`
2. Make your changes
3. Run the full test suite:
   ```bash
   cargo fmt --check
   cargo check
   cargo test
   cargo clippy --all-targets --all-features -- -D warnings

   cd dashboard
   npm run lint
   npm run type-check
   npm run build
   npm run test:e2e -- first-launch-onboarding.spec.ts
   npm run test:release-smoke

   cd ..
   cargo build --release
   ```
4. Open a Pull Request against `main`

---

## Code Style

**Rust**
- `cargo fmt` before every commit (enforced by CI)
- `cargo clippy --all-targets --all-features -- -D warnings` must pass (zero warnings)
- Errors via `thiserror` + `anyhow`; no `.unwrap()` in production paths
- DuckDB SQL: use `?1`, `?2` positional params; never string-interpolate user input
- Public self-hosted code must not add Clerk, Stripe, ClickHouse runtime,
  private ops config, or secrets. Cloud integrations belong in the private
  `sparklytics-cloud` repository behind existing integration boundaries.

**TypeScript (Dashboard + SDK)**
- `strict: true` in all `tsconfig.json` files
- No `any` types
- All browser API access inside `useEffect` or guarded by `typeof window !== 'undefined'`

---

## Testing

| Layer | Command | Coverage |
|-------|---------|---------|
| Rust unit tests | `cargo test` | Core logic, visitor ID, config |
| Rust integration tests | `cargo test` | HTTP routes, auth, security |
| Dashboard lint/type/build | `cd dashboard && npm run lint && npm run type-check && npm run build` | Static dashboard checks |
| Browser smoke | `cd dashboard && npm run test:e2e -- first-launch-onboarding.spec.ts && npm run test:release-smoke` | First-launch and release flow |
| SDK unit tests | `cd sdk/next && npm test` | `@sparklytics/next` package tests |

All tests must pass before a PR is merged.

---

## Nested Repositories

The local workspace can contain nested repositories:

- `cloud/` — private cloud runtime
- `docs/` — public docs repository
- `marketing/` — public marketing site
- `sdk/next/` — public `@sparklytics/next` package

Before committing, verify which repository owns the files you changed. From the
root public runtime repo, do not use `git add -A` or `git add .`; stage explicit
paths only.

---

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add UTM campaign filter to stats endpoint
fix: correct CORS header for /api/collect preflight
docs: update SDK README with npm link workflow
chore: bump duckdb to 1.1.0
```

---

## Reporting Issues

- **Bugs**: Open a GitHub issue with reproduction steps and expected vs actual behaviour
- **Security vulnerabilities**: See [SECURITY.md](SECURITY.md) — do not open a public issue

---

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
