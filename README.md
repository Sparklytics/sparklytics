# Sparklytics

**Developer-first analytics. Zero config. Full control.**

> Work in progress — Sprint 0 scaffolding.

Sparklytics is an open-source, self-hosted web analytics platform built in Rust. It is lightweight, privacy-respecting, and ships as a single binary with an embedded React dashboard.

## Status

This repository is under active development. See [`docs/`](docs/) for the full specification, architecture decisions, and sprint plans.

| Sprint | Focus | Status |
|--------|-------|--------|
| 0 | Rust workspace, event collection, DuckDB | In Progress |
| 1 | Query API, sessions, self-hosted auth | Not Started |
| 2 | React dashboard | Not Started |
| 3 | `@sparklytics/next` npm SDK | Not Started |
| 4 | OSS launch, load tests, Docker | Not Started |
| 5 | Clerk auth, cloud, PostgreSQL | Not Started |

## Tech Stack

- **Backend**: Rust (Axum 0.8, Tokio), DuckDB (self-hosted)
- **Frontend**: React 18 + Vite + TailwindCSS + shadcn/ui
- **SDK**: `@sparklytics/next` (npm, Next.js App + Pages Router)
- **Auth**: Argon2id password hashing, JWT HttpOnly cookies

## Documentation

Full documentation is in [`docs/`](docs/). Start with [`docs/INDEX.md`](docs/INDEX.md).

## License

MIT
