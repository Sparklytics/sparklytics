# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x | ✅ Active |

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Send a detailed report to: **security@sparklytics.dev**

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Any suggested mitigations

We will acknowledge your report within 48 hours and aim to issue a patch within 7 days for critical issues.

## Disclosure Policy

We follow [responsible disclosure](https://en.wikipedia.org/wiki/Responsible_disclosure):

1. You report the issue privately
2. We confirm receipt and investigate
3. We develop and test a fix
4. We release the fix and credit you (if you wish) in the changelog
5. We publish a security advisory

## Security Design

Sparklytics is designed with security in mind:

- **No cookies** — visitor tracking is stateless and privacy-preserving
- **Parameterized queries** — all DuckDB queries use positional parameters; no string interpolation of user input
- **Payload limits** — 100KB total body, 4KB per `event_data` field
- **Rate limiting** — 60 req/min per IP on `/api/collect`
- **CORS isolation** — collection endpoint allows `*`; analytics query endpoints enforce `SPARKLYTICS_CORS_ORIGINS`
- **Auth** — Argon2id password hashing, JWT HttpOnly + SameSite=Strict cookies
- **API keys** — SHA-256 hashed, never stored raw
- **Non-root container** — Docker image runs as `nonroot` (uid 65532)
- **Static binary** — minimal attack surface, no interpreter or runtime dependencies
