#!/bin/sh
set -eu

ADMIN_EMAIL="${INIT_ADMIN_EMAIL:-admin@example.com}"
ADMIN_PASSWORD="${INIT_ADMIN_PASSWORD:-admin1234}"
ADMIN_FULL_NAME="${INIT_ADMIN_FULL_NAME:-Default Admin}"

psql -v ON_ERROR_STOP=1 \
  --username "$POSTGRES_USER" \
  --dbname "$POSTGRES_DB" <<SQL
INSERT INTO users (email, full_name, password_hash, is_admin, active)
VALUES (
  '${ADMIN_EMAIL}',
  '${ADMIN_FULL_NAME}',
  crypt('${ADMIN_PASSWORD}', gen_salt('bf')),
  TRUE,
  TRUE
)
ON CONFLICT (email) DO UPDATE
SET
  full_name = EXCLUDED.full_name,
  is_admin = TRUE,
  active = TRUE,
  updated_at = NOW();
SQL
