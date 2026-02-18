---
name: new-route
description: Scaffold a complete, production-quality Axum route for Sparklytics. Generates the handler module, error type, router registration snippet, and integration test skeleton — all following Rust and REST best practices. Usage: /new-route <METHOD> <path> [sprint-number]  e.g. /new-route POST /api/collect 0
---

The user wants to scaffold a new Axum route: $ARGUMENTS

Parse $ARGUMENTS as: <HTTP_METHOD> <path> [sprint_number]

## Step 1: Read the spec

If a sprint number was provided, read `docs/sprints/sprint-$SPRINT.md` and find the contract for this endpoint. Also read `docs/sprints/sprint-index.md` for the full request/response shape.

If no sprint number was provided, ask the user to describe the request body, response body, and auth requirement before generating code.

## Step 2: Generate the handler module

Produce a complete Rust module file. Follow every convention below — do not skip any section.

---

### Rust + Axum Conventions

**Error handling**
- Define a local `AppError` enum using `thiserror::Error` if one doesn't already exist in `crates/sparklytics-server/src/error.rs`
- Every variant maps to an HTTP status code in the `IntoResponse` impl
- Never use `.unwrap()`, `.expect()`, or `panic!()` — always propagate with `?` or map to `AppError`
- Database errors → `AppError::Internal` (log with `tracing::error!`, return 500 to client)
- Validation errors → `AppError::BadRequest(String)` (return 400 with message)
- Not found → `AppError::NotFound` (return 404)
- Unauthorized → `AppError::Unauthorized` (return 401)

**Request/Response types**
- Request structs: `#[derive(Debug, Deserialize)]` + `#[serde(deny_unknown_fields)]`
- Response structs: `#[derive(Debug, Serialize)]`
- Use `Option<T>` for optional fields, never use `serde(default)` silently swallowing missing data
- Validate inputs explicitly: check non-empty strings, numeric ranges, URL format, etc.
- Never trust `Content-Length` — always use `Json<T>` extractor which has a built-in body size limit

**Handler signature**
```rust
#[tracing::instrument(skip(state), fields(website_id = %payload.website_id))]
pub async fn handler_name(
    State(state): State<Arc<AppState>>,
    // Path(id): Path<Uuid>,        // for path params
    // Query(params): Query<Params>, // for query params
    Json(payload): Json<RequestType>,
) -> Result<impl IntoResponse, AppError> {
```

**State access**
- Always `Arc<AppState>` — never clone the whole state, only clone `Arc`
- DB pool accessed via `state.db` — never open a new connection per request
- Config accessed via `state.config` — never read env vars inside handlers

**Response conventions**
- 201 Created: `(StatusCode::CREATED, Json(response))`
- 204 No Content: `StatusCode::NO_CONTENT`
- 200 OK: `Json(response)` (axum defaults to 200)
- Error envelope must be `{ "error": "...", "code": "SCREAMING_SNAKE_CASE" }`

**Performance**
- Avoid `.clone()` on large types — prefer `Arc` or references
- Don't hold locks across `.await` points
- Use `tokio::spawn` for fire-and-forget background work, not blocking the handler
- For `/api/collect`: buffer events and write in batches — never write one row per request

**Tracing & observability**
- `#[tracing::instrument]` on every public handler
- `tracing::info!` for successful operations at INFO level
- `tracing::warn!` for handled errors (bad input, 404) at WARN level
- `tracing::error!` for internal failures (DB errors, unexpected states) at ERROR level
- Never log PII: no IP addresses, no user agents, no email addresses

**Security**
- Reject oversized payloads (Axum's `DefaultBodyLimit` layer handles this — confirm it's set)
- Sanitize any string that will be used in SQL (prefer parameterized queries — never string-format SQL)
- Rate-limited endpoints: confirm Tower rate-limit layer is applied at router level
- CORS: only return `Access-Control-Allow-Origin` headers on explicitly allowed endpoints

---

## Step 3: Output

Produce these sections in order:

### `crates/sparklytics-server/src/routes/<name>.rs`
Full handler module with imports, request/response types, validation, handler function, and `pub fn router() -> Router<Arc<AppState>>`.

### Router registration snippet
The line(s) to add in `crates/sparklytics-server/src/routes/mod.rs` to mount this router.

### Integration test skeleton
```rust
#[cfg(test)]
mod tests {
    // Use axum_test or axum::test helpers
    // Cover: happy path, missing fields, wrong auth, edge cases from BDD spec
}
```

### Checklist before marking done
- [ ] `cargo fmt` passes
- [ ] `cargo clippy -- --deny warnings --deny clippy::unwrap_used` passes
- [ ] Contract checker subagent run (`/agent:api-contract-checker`)
- [ ] Security reviewer run if handler touches auth/tenant logic (`/agent:security-reviewer`)
