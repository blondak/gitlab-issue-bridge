# Production Deployment

This guide is for operators who deploy and run IssueHub on a production server.
Repository and release-process documentation is in [DEVELOPER.md](DEVELOPER.md).

## Overview

IssueHub runs as a Docker Compose stack:

- `traefik` is the only public entrypoint and terminates TLS.
- `frontend` serves the React SPA.
- `api` serves the HTTP API, health checks, metrics, and upload/download endpoints.
- `worker` processes PostgreSQL-backed jobs.
- `postgres` stores application data and the job queue.

The production server should pull already published images from GHCR. It should not build application images locally.

Default production images:

```text
ghcr.io/<owner>/issuehub-api:latest
ghcr.io/<owner>/issuehub-worker:latest
ghcr.io/<owner>/issuehub-frontend:latest
```

Use a release tag instead of `latest` when you need a pinned deployment or rollback. If you use a pinned tag, keep the same tag for `api`, `worker`, and `frontend`.

## 1. Server Requirements

Prepare:

- a Linux server with Docker Engine and the Docker Compose plugin,
- DNS A/AAAA record for the public domain,
- inbound ports `80` and `443` open for Traefik and Let's Encrypt,
- a GHCR token with package read access,
- SMTP credentials for invitations and password recovery,
- persistent storage for PostgreSQL and local authoritative attachments,
- backup storage outside the server.

Only Traefik should publish host ports. Do not publish `api`, `frontend`, `worker`, or `postgres` directly.

## 2. Install Files

The server needs the production Compose file, Traefik config, and `.env.example`. IssueHub schema migrations and the bootstrap admin user are handled by the API on startup, so PostgreSQL init scripts do not need to be mounted from the host.

```bash
mkdir -p /opt/issuehub
cd /opt/issuehub

git clone <REPOSITORY_URL> . --depth=1
cp .env.example .env
```

Open `.env` and replace development defaults with production values:

```bash
nano .env
```

## 3. Configure Environment

Required values:

| Variable | Description | Example or generation |
|---|---|---|
| `DOMAIN` | Public application domain. | `issuehub.example.com` |
| `ACME_EMAIL` | Email for Let's Encrypt notifications. | Operations email |
| `FRONTEND_ORIGIN` | Browser origin allowed by the API. | `https://issuehub.example.com` |
| `PUBLIC_FRONTEND_URL` | Public URL used in email links. | `https://issuehub.example.com` |
| `POSTGRES_DB` | Database name. | `issuehub` |
| `POSTGRES_USER` | Database user. | `issuehub` |
| `POSTGRES_PASSWORD` | Strong database password. | `openssl rand -hex 16` |
| `POSTGRES_DATA_DIR` | Persistent PostgreSQL data directory. | `/var/lib/issuehub/postgres` |
| `ATTACHMENTS_HOST_DIR` | Persistent local attachment directory. | `/var/lib/issuehub/attachments` |
| `ATTACHMENT_CACHE_DIR` | Rehydratable GitLab attachment cache. | `/var/cache/issuehub/gitlab-attachments` |
| `SECRET_ENCRYPTION_KEY` | Base64 key for encrypted GitLab tokens and webhook secrets. | `openssl rand -base64 32` |
| `GITHUB_REPOSITORY_OWNER` | GHCR owner for published images. | GitHub username or organization |
| `INIT_ADMIN_EMAIL` | Initial admin email, used on first startup. | `admin@example.com` |
| `INIT_ADMIN_PASSWORD` | Initial admin password, used on first startup. | strong generated password |
| `INIT_ADMIN_FULL_NAME` | Initial admin display name. | `IssueHub Admin` |
| `SMTP_HOST` | SMTP server hostname. | SMTP provider |
| `SMTP_USERNAME` | SMTP username. | SMTP provider |
| `SMTP_PASSWORD` | SMTP password. | SMTP provider |
| `SMTP_FROM_EMAIL` | Sender email address. | verified sender |

Production security settings:

```env
SESSION_COOKIE_SECURE=true
DEBUG=false
REQUIRE_SECRET_ENCRYPTION_KEY=true
RATE_LIMIT_ENABLED=true
```

When `DEBUG=true`, the API returns internal `500` error details in JSON responses. Use it only for local debugging or a short controlled production investigation.

Recommended production limits:

```env
WORKER_HEARTBEAT_INTERVAL_SECONDS=15
WORKER_STALE_AFTER_SECONDS=60
QUEUE_STALE_AFTER_SECONDS=300
RATE_LIMIT_WINDOW_SECONDS=900
RATE_LIMIT_LOGIN_PER_EMAIL=5
RATE_LIMIT_LOGIN_PER_IP=50
RATE_LIMIT_PASSWORD_RECOVERY_PER_EMAIL=3
RATE_LIMIT_PASSWORD_RECOVERY_PER_IP=20
RATE_LIMIT_INVITATION_RESEND_PER_ADMIN=10
RATE_LIMIT_UPLOADS_PER_USER=60
UPLOAD_MAX_BYTES=10485760
UPLOAD_ALLOWED_CONTENT_TYPES=image/png,image/jpeg,image/gif,image/webp,application/pdf,text/plain,text/csv,application/json,application/zip
```

`UPLOAD_ALLOWED_CONTENT_TYPES` is an allowlist. Add new MIME types explicitly when needed; do not use `*` in production.

Storage rules:

- `ATTACHMENTS_HOST_DIR` stores local authoritative attachments and must be backed up.
- `ATTACHMENT_CACHE_DIR` stores GitLab-synchronized attachment cache and may be non-persistent because IssueHub can rehydrate those files from GitLab on download.
- PostgreSQL data must be persistent and backed up.

## 4. Start the Stack

Log in to GHCR once:

```bash
echo <GITHUB_PAT> | docker login ghcr.io -u <GITHUB_USERNAME> --password-stdin
```

Validate the Compose configuration:

```bash
docker compose -f docker-compose.prod.yml config
```

Pull images and start:

```bash
docker compose -f docker-compose.prod.yml pull
docker compose -f docker-compose.prod.yml up -d
```

Verify:

```bash
docker compose -f docker-compose.prod.yml ps
docker compose -f docker-compose.prod.yml logs --tail=50
curl -fsS https://<DOMAIN>/health/live
curl -fsS https://<DOMAIN>/health/ready
curl -fsS https://<DOMAIN>/metrics
```

The bootstrap admin account is created automatically on first startup from `INIT_ADMIN_EMAIL`, `INIT_ADMIN_PASSWORD`, and `INIT_ADMIN_FULL_NAME`.

## 5. Health and Monitoring

Available endpoints:

| Endpoint | Purpose |
|---|---|
| `/health/live` | Process liveness. |
| `/health/ready` | Readiness check for PostgreSQL, migrated schema state, queue health, and worker heartbeat. |
| `/metrics` | Prometheus metrics for queue state, workers, SMTP/webhook failures, and database connectivity. |
| `/api/v1/admin/health` | Admin-only operational JSON used by the Health UI page. |

Monitor at least:

- readiness status,
- pending and dead jobs,
- worker heartbeat freshness,
- database connectivity,
- SMTP failures,
- webhook failures,
- disk usage for PostgreSQL and attachments.

## 6. Updates

Update to the current published production images:

```bash
cd /opt/issuehub

docker compose -f docker-compose.prod.yml pull
docker compose -f docker-compose.prod.yml up -d --remove-orphans
curl -fsS https://<DOMAIN>/health/ready
```

After an update, check:

- login,
- project list,
- issue list,
- issue detail,
- local attachment download,
- GitLab-synchronized attachment download if GitLab integration is used,
- Health page in the admin UI.

## 7. Rollback

Before rollback, verify database migration compatibility with the target release. If a release includes irreversible migrations, restore from backup or deploy a forward-fix release instead of blindly downgrading binaries.

For a pinned rollback, run all three application services with the same release tag. One practical option is a local override file saved as `docker-compose.rollback.yml`:

```yaml
services:
  api:
    image: ghcr.io/<owner>/issuehub-api:v1.0.0
  worker:
    image: ghcr.io/<owner>/issuehub-worker:v1.0.0
  frontend:
    image: ghcr.io/<owner>/issuehub-frontend:v1.0.0
```

Then start with both Compose files:

```bash
docker compose -f docker-compose.prod.yml -f docker-compose.rollback.yml pull
docker compose -f docker-compose.prod.yml -f docker-compose.rollback.yml up -d --remove-orphans
curl -fsS https://<DOMAIN>/health/ready
```

Remove the override when returning to `latest`.

## 8. Server Operations

Common commands:

```bash
# Logs
docker compose -f docker-compose.prod.yml logs -f api
docker compose -f docker-compose.prod.yml logs -f worker

# Restart one service
docker compose -f docker-compose.prod.yml restart api

# Stop the stack
docker compose -f docker-compose.prod.yml down

# Free disk space from old images
docker image prune -f
```

If readiness fails, inspect:

```bash
docker compose -f docker-compose.prod.yml ps
docker compose -f docker-compose.prod.yml logs --tail=200 api
docker compose -f docker-compose.prod.yml logs --tail=200 worker
docker compose -f docker-compose.prod.yml logs --tail=200 postgres
```

## 9. Backups

Data lives on the host in directories configured through `.env`.

| Path variable | Content | Backup requirement |
|---|---|---|
| `POSTGRES_DATA_DIR` | PostgreSQL database. | Required |
| `ATTACHMENTS_HOST_DIR` | Local authoritative attachments and temporary uploads. | Required |
| `ATTACHMENT_CACHE_DIR` | Rehydratable GitLab attachment cache. | Optional |

Database backup:

```bash
docker compose -f docker-compose.prod.yml exec postgres \
  sh -c 'pg_dump -U "$POSTGRES_USER" "$POSTGRES_DB"' | gzip > backup_$(date +%Y%m%d).sql.gz
```

Attachment backup:

```bash
ATTACHMENTS_DIR=/var/lib/issuehub/attachments
tar -czf attachments_$(date +%Y%m%d).tar.gz -C "$(dirname "$ATTACHMENTS_DIR")" "$(basename "$ATTACHMENTS_DIR")"
```

Do not rely on `ATTACHMENT_CACHE_DIR` for authoritative data. It can be rebuilt from GitLab for synchronized attachments.

## 10. Restore

Stop API, worker, and frontend before restoring database or authoritative attachment files:

```bash
docker compose -f docker-compose.prod.yml stop api worker frontend
```

Restore PostgreSQL from a dump:

```bash
gzip -dc backup_YYYYMMDD.sql.gz | docker compose -f docker-compose.prod.yml exec -T postgres \
  sh -c 'psql -U "$POSTGRES_USER" "$POSTGRES_DB"'
```

Restore authoritative attachments into `ATTACHMENTS_HOST_DIR`, then restart:

```bash
docker compose -f docker-compose.prod.yml up -d
curl -fsS https://<DOMAIN>/health/ready
```

After restore, check:

- login,
- project and issue list,
- local attachment download,
- GitLab-synchronized attachment download,
- worker heartbeat,
- pending and dead jobs.
