# Contributing to Sparklytics

Thank you for your interest in contributing. This document covers setup, workflow, and code style.

---

## Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | stable (≥ 1.82) | Backend |
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

# Dashboard (runs at :3001, proxies /api to :3000)
cd dashboard && npm ci && npm run dev

# SDK
cd sdk && npm ci && npm run build && npm test
```

---

## Development Workflow

1. Fork the repo and create a branch: `git checkout -b feat/my-feature`
2. Make your changes
3. Run the full test suite:
   ```bash
   cargo fmt --all -- --check
   cargo clippy -- -D warnings
   cargo test
   cd dashboard && npm run build
   cd sdk && npm test && npm run build
   ```
4. Open a Pull Request against `main`

---

## Code Style

**Rust**
- `cargo fmt` before every commit (enforced by CI)
- `cargo clippy -- -D warnings` must pass (zero warnings)
- Errors via `thiserror` + `anyhow`; no `.unwrap()` in production paths
- DuckDB SQL: use `?1`, `?2` positional params; never string-interpolate user input

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
| SDK unit tests | `cd sdk && npm test` | All BDD scenarios (21 tests) |

All tests must pass before a PR is merged.

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
