---
name: sprint-scaffold
description: Read a sprint spec and produce a concrete implementation plan with file paths, function signatures, SQL schemas, and a BDD test checklist. Usage: /sprint-scaffold <number>  e.g. /sprint-scaffold 0
---

The user wants to start Sprint $ARGUMENTS.

1. Read `docs/sprints/sprint-$ARGUMENTS.md` in full.
2. Also read the relevant sections of `docs/sprints/sprint-index.md` (data model + API contracts).
3. Cross-reference `CLAUDE.md` critical facts that apply to this sprint.

Then produce a structured implementation plan:

## Sprint $ARGUMENTS Implementation Plan

### Files to Create
List every file to create with its exact path per the workspace layout in CLAUDE.md:
- `crates/<crate>/src/<module>.rs` — purpose
- `dashboard/src/<path>.tsx` — purpose
- etc.

### Rust Structs & Functions
For each new Rust module, list:
- Struct definitions (with field names and types from the spec)
- Key function signatures (pub fn name(args) -> Result<T, E>)
- Trait implementations required

### Database / SQL
- Any DuckDB table CREATE statements (copy exact schema from sprint spec)
- Any PostgreSQL migration SQL needed
- Any ClickHouse table definitions

### API Endpoints
For each endpoint in this sprint:
- Method + path
- Request body shape (JSON)
- Response body shape (JSON)
- Auth requirement

### BDD Scenarios to Implement as Tests
List each Gherkin scenario from the sprint doc as a test function name:
- `test_<scenario_slug>` — one-line description

### Critical Facts to Watch
List any CLAUDE.md critical facts that are especially relevant to this sprint (e.g. tenant_id=NULL, rate limit=60/min, visitor ID formula).

### Suggested Implementation Order
A numbered sequence that avoids blocked dependencies (e.g. core types before server, schema before queries).
