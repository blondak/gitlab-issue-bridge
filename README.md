# IssueHub

IssueHub is an internal issue management system with project-level and issue-level permissions, local-first workflows, and optional GitLab integration. GitLab is configured per project and is never required for the core issue workflow.

Project documentation is maintained in English.

## Documentation

- [DEPLOY.md](DEPLOY.md): production deployment and operations for server operators.

## Services

- `frontend`: React + Mantine SPA for issues, comments, access management, health, and administration.
- `api`: Rust Axum API for authentication, projects, issues, permissions, uploads, GitLab integration, jobs, health, and metrics.
- `worker`: Rust worker that processes the PostgreSQL-backed queue.
- `postgres`: primary database and job queue storage.
- `traefik`: reverse proxy that serves the frontend, API, OpenAPI docs, health, and metrics from one public origin.

## Quick Start

1. Copy `.env.example` to `.env`.
2. Optionally adjust `POSTGRES_DATA_DIR` and attachment paths if persistent data should live outside the default `./.data/...` directories.
3. Start the stack:

```sh
docker compose up --build
```

4. Open `http://localhost:3000`, or the port configured with `TRAEFIK_PORT`.

If no admin user exists, the bootstrap admin is created from:

```env
INIT_ADMIN_EMAIL=admin@example.com
INIT_ADMIN_PASSWORD=admin1234
INIT_ADMIN_FULL_NAME=Default Admin
```

Change these values before exposing the stack outside a local development environment.

## Validation

The CI workflow runs:

- `cargo check --locked`
- `cargo test --locked`
- `npm test`
- `npm run build`
- `docker compose config`

Backend integration tests require PostgreSQL in `DATABASE_URL`:

```sh
DATABASE_URL=postgres://issuehub_test:issuehub_test@localhost:5432/issuehub_test cargo test
```

Frontend tests use the built-in Node test runner:

```sh
cd frontend
npm test
```

End-to-end smoke test against a real Docker Compose stack:

```sh
cp .env.example .env
SMOKE_START_STACK=1 ./scripts/smoke-compose.sh
```

The smoke script covers the local-first workflow, permission matrix, comments, attachments, GitLab-disabled webhook enqueue, admin job list, and worker retry/dead/stale queue behavior.

Security-focused smoke test with low limits:

```sh
ISSUEHUB_SMOKE_SECURITY_LIMITS=1 SMOKE_START_STACK=1 ./scripts/smoke-compose.sh
```

It verifies rate limiting, upload hardening, and the `SECRET_ENCRYPTION_KEY` startup guard.

## Error Details

By default, API responses for internal `500` errors return only a generic JSON message:

```json
{ "message": "Internal server error" }
```

For local debugging, detailed internal error messages can be enabled with:

```env
DEBUG=true
```

Production deployments must keep:

```env
DEBUG=false
```

## Persistent Data and Storage

Persistent data is configured through `.env`:

- `POSTGRES_DATA_DIR`: host directory for PostgreSQL data.
- `ATTACHMENTS_HOST_DIR`: host directory for local authoritative attachments and temporary uploads.
- `ATTACHMENT_CACHE_DIR`: cache for GitLab-synchronized attachments. This directory is not authoritative; if a cached file is lost, the API downloads it again from GitLab through the server-side proxy on the next request.
- `TEMP_UPLOAD_RETENTION_HOURS`: retention window for unsent temporary uploads before cleanup.
- `WORKER_HEARTBEAT_INTERVAL_SECONDS`: worker heartbeat write interval.
- `WORKER_STALE_AFTER_SECONDS`: heartbeat age after which a worker is treated as unhealthy.
- `QUEUE_STALE_AFTER_SECONDS`: age after which a `processing` job is treated as stale.
- `DEBUG`: enables detailed internal `500` errors when set to `true`; keep `false` in production.
- `REQUIRE_SECRET_ENCRYPTION_KEY`: when `true`, API and worker refuse to start without a valid `SECRET_ENCRYPTION_KEY`.
- `RATE_LIMIT_ENABLED`, `RATE_LIMIT_WINDOW_SECONDS`: global application rate limiting switch and window.
- `RATE_LIMIT_LOGIN_PER_EMAIL`, `RATE_LIMIT_LOGIN_PER_IP`: login limits.
- `RATE_LIMIT_PASSWORD_RECOVERY_PER_EMAIL`, `RATE_LIMIT_PASSWORD_RECOVERY_PER_IP`: password recovery limits.
- `RATE_LIMIT_INVITATION_RESEND_PER_ADMIN`: invitation resend limit per admin.
- `RATE_LIMIT_UPLOADS_PER_USER`: upload limit per user in one window.
- `UPLOAD_MAX_BYTES`: maximum size of one upload.
- `UPLOAD_ALLOWED_CONTENT_TYPES`: comma-separated upload MIME type allowlist.
- `TRAEFIK_PORT`: public port for the whole stack.
- `INIT_ADMIN_EMAIL`: bootstrap admin email.
- `INIT_ADMIN_PASSWORD`: bootstrap admin password.
- `INIT_ADMIN_FULL_NAME`: bootstrap admin display name.
- `FRONTEND_ORIGIN`: origin allowed by API CORS.
- `PUBLIC_FRONTEND_URL`: public frontend URL used in invitation and recovery links.
- `SECRET_ENCRYPTION_KEY`: base64 key for encrypting stored secrets.
- `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`: SMTP connection settings.
- `SMTP_FROM_EMAIL`, `SMTP_FROM_NAME`: invitation email sender identity.
- `SMTP_STARTTLS`: whether to use STARTTLS for SMTP.

Example:

```env
POSTGRES_DATA_DIR=/srv/issue-bridge/postgres
ATTACHMENTS_HOST_DIR=/srv/issue-bridge/attachments
ATTACHMENT_CACHE_DIR=/var/cache/issuehub/gitlab-attachments
TEMP_UPLOAD_RETENTION_HOURS=24
```

Docker Compose mounts PostgreSQL data into `/var/lib/postgresql/data` and attachments into `/var/lib/issuehub/attachments`.

Local IssueHub attachments are authoritative data and must be included in backups. GitLab-synchronized attachments are stored only as cache under `ATTACHMENT_CACHE_DIR`.

## Health and Metrics

- `/health`: simple backward-compatible liveness response.
- `/health/live`: API process liveness.
- `/health/ready`: readiness check for PostgreSQL, schema state, queue, and worker heartbeat. Returns `503` when any check fails.
- `/metrics`: Prometheus text metrics for database connectivity, queue depth, stale/dead jobs, SMTP failures, webhook failures, and worker heartbeats.
- `/api/v1/admin/health`: admin-only JSON health overview with checks, queue status, workers, and recent failed/dead jobs.

## Frontend and Routing

Traefik serves the frontend, API, `/api-docs`, health endpoints, and metrics from a single origin. The frontend should normally use relative `/api` calls.

This stack uses a static Traefik file provider from [infra/traefik/dynamic.yml](infra/traefik/dynamic.yml), not the Docker socket. This avoids host Docker API compatibility problems.

Frontend-related variables:

- `FRONTEND_ORIGIN`: origin allowed by API CORS.
- `PUBLIC_FRONTEND_URL`: public URL inserted into email links; it can differ from internal origins.
- `SECRET_ENCRYPTION_KEY`: base64 key used to encrypt GitLab tokens and webhook secrets.
- `VITE_API_BASE_URL`: optional explicit API URL for the frontend; empty means relative calls.
- `VITE_DEV_API_PROXY_TARGET`: Vite development proxy target for `/api`, `/health`, `/api-docs`, and `/metrics`.

## GitLab Integration

GitLab is configured in the application at project level, not globally in `.env`.

The model is:

- an admin creates an internal project,
- a global admin or project admin configures GitLab integration on that project,
- IssueHub synchronizes issues, comments, and attachment references through that project integration.

This allows one IssueHub instance to manage multiple projects, each with its own GitLab host, external project ID, token, and webhook secret.

GitLab tokens and webhook secrets are encrypted before storage. Existing plaintext values are migrated into encrypted columns by database migrations.

## SMTP and Invitations

Admins can create user invitations from the UI. The API creates an invitation record and enqueues an email job; the worker sends the email through SMTP.

Minimal production SMTP configuration:

```env
PUBLIC_FRONTEND_URL=https://bridge.example.com
SECRET_ENCRYPTION_KEY=<base64-32-byte-key>
SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_USERNAME=bridge@example.com
SMTP_PASSWORD=secret
SMTP_FROM_EMAIL=bridge@example.com
SMTP_FROM_NAME=IssueHub
SMTP_STARTTLS=true
```

If SMTP is not configured, invitation email jobs fail and remain in the queue with an error.

## Current Capabilities

The repository currently includes:

- Docker Compose infrastructure for local and production-style deployment.
- PostgreSQL schema and migrations for users, invitations, sessions, projects, GitLab integrations, issues, comments, attachments, permissions, jobs, audit logs, metrics, and worker heartbeats.
- Local-first project, issue, comment, attachment, access, invitation, and authentication workflows.
- Optional per-project GitLab integration for issue/comment synchronization, attachment proxying, and webhook enqueueing.
- PostgreSQL-backed worker queue for email delivery, cleanup, webhook processing, and retry/dead job handling.
- OpenAPI documentation at `/api-docs`.
- React + Mantine frontend with capability-driven navigation, issue detail, threaded comments, project/user/admin access management, health UI, light/dark/automatic theme, and toast notifications.
