#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require_cmd curl
require_cmd docker
require_cmd jq
require_cmd mktemp
require_cmd timeout

env_value() {
  local key="$1"
  local default_value="$2"
  local value=""

  value="$(printenv "$key" || true)"
  if [[ -n "$value" ]]; then
    printf '%s' "$value"
    return
  fi

  if [[ -f .env ]]; then
    value="$(sed -n "s/^${key}=//p" .env | tail -n 1)"
  fi

  if [[ -n "$value" ]]; then
    printf '%s' "$value"
  else
    printf '%s' "$default_value"
  fi
}

bool_enabled() {
  case "$1" in
    1|true|TRUE|yes|YES|on|ON) return 0 ;;
    *) return 1 ;;
  esac
}

SECURITY_SMOKE="${ISSUEHUB_SMOKE_SECURITY_LIMITS:-0}"
if bool_enabled "$SECURITY_SMOKE"; then
  export RATE_LIMIT_ENABLED="${RATE_LIMIT_ENABLED:-true}"
  export RATE_LIMIT_WINDOW_SECONDS="${RATE_LIMIT_WINDOW_SECONDS:-60}"
  export RATE_LIMIT_LOGIN_PER_EMAIL="${RATE_LIMIT_LOGIN_PER_EMAIL:-2}"
  export RATE_LIMIT_LOGIN_PER_IP="${RATE_LIMIT_LOGIN_PER_IP:-100}"
  export RATE_LIMIT_PASSWORD_RECOVERY_PER_EMAIL="${RATE_LIMIT_PASSWORD_RECOVERY_PER_EMAIL:-2}"
  export RATE_LIMIT_PASSWORD_RECOVERY_PER_IP="${RATE_LIMIT_PASSWORD_RECOVERY_PER_IP:-100}"
  export RATE_LIMIT_INVITATION_RESEND_PER_ADMIN="${RATE_LIMIT_INVITATION_RESEND_PER_ADMIN:-2}"
  export RATE_LIMIT_UPLOADS_PER_USER="${RATE_LIMIT_UPLOADS_PER_USER:-5}"
  export UPLOAD_MAX_BYTES="${UPLOAD_MAX_BYTES:-64}"
  export UPLOAD_ALLOWED_CONTENT_TYPES="${UPLOAD_ALLOWED_CONTENT_TYPES:-image/png,image/jpeg,image/gif,image/webp,application/pdf,text/plain,text/csv,application/json,application/zip}"
fi

TRAEFIK_PORT="$(env_value TRAEFIK_PORT 3000)"
BASE_URL="${ISSUEHUB_BASE_URL:-http://localhost:${TRAEFIK_PORT}}"
ORIGIN="${ISSUEHUB_ORIGIN:-$(env_value FRONTEND_ORIGIN "http://localhost:${TRAEFIK_PORT}")}"
DB_NAME="$(env_value POSTGRES_DB issue_bridge)"
DB_USER="$(env_value POSTGRES_USER issue_bridge)"

RUN_ID="${ISSUEHUB_SMOKE_RUN_ID:-$(date +%Y%m%d%H%M%S)-$$}"
SMOKE_PREFIX="phase3-${RUN_ID}"
PROJECT_SLUG="${SMOKE_PREFIX}"
SMOKE_PASSWORD="${ISSUEHUB_SMOKE_PASSWORD:-Phase3SmokePassword123!}"
WEBHOOK_SECRET="${SMOKE_PREFIX}-webhook-secret"
GITLAB_PROJECT_ID=424242

TMP_DIR="$(mktemp -d)"
ADMIN_COOKIE="${TMP_DIR}/admin.cookie"
VIEW_COOKIE="${TMP_DIR}/view.cookie"
CREATE_COOKIE="${TMP_DIR}/create.cookie"
READ_COOKIE="${TMP_DIR}/read.cookie"
COMMENT_COOKIE="${TMP_DIR}/comment.cookie"
EDIT_COOKIE="${TMP_DIR}/edit.cookie"
ISSUE_ADMIN_COOKIE="${TMP_DIR}/issue-admin.cookie"
PROJECT_ADMIN_COOKIE="${TMP_DIR}/project-admin.cookie"
RATE_COOKIE="${TMP_DIR}/rate.cookie"
LAST_RESPONSE=""

ADMIN_EMAIL="${SMOKE_PREFIX}-admin@example.invalid"
RATE_EMAIL="${SMOKE_PREFIX}-rate@example.invalid"
VIEW_EMAIL="${SMOKE_PREFIX}-view@example.invalid"
CREATE_EMAIL="${SMOKE_PREFIX}-create@example.invalid"
READ_EMAIL="${SMOKE_PREFIX}-read@example.invalid"
COMMENT_EMAIL="${SMOKE_PREFIX}-comment@example.invalid"
EDIT_EMAIL="${SMOKE_PREFIX}-edit@example.invalid"
ISSUE_ADMIN_EMAIL="${SMOKE_PREFIX}-issue-admin@example.invalid"
PROJECT_ADMIN_EMAIL="${SMOKE_PREFIX}-project-admin@example.invalid"

log() {
  printf '[smoke] %s\n' "$*"
}

fail() {
  printf '[smoke] ERROR: %s\n' "$*" >&2
  exit 1
}

psql_exec() {
  docker compose exec -T postgres psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 "$@"
}

psql_scalar() {
  psql_exec -qAt "$@" | tr -d '\r' | sed -n '1p'
}

cleanup() {
  local status=$?

  if [[ "${KEEP_SMOKE_DATA:-0}" == "1" ]]; then
    log "keeping smoke data because KEEP_SMOKE_DATA=1"
    rm -rf "$TMP_DIR"
    exit "$status"
  fi

  psql_exec \
    -v project_slug="$PROJECT_SLUG" \
    -v email_like="${SMOKE_PREFIX}-%@example.invalid" \
    <<'SQL' >/dev/null 2>&1 || true
WITH smoke_project AS (
  SELECT id FROM projects WHERE slug = :'project_slug'
),
smoke_jobs AS (
  SELECT id
  FROM jobs
  WHERE payload->>'project_id' IN (SELECT id::text FROM smoke_project)
     OR dedupe_key LIKE :'project_slug' || '%'
     OR dedupe_key LIKE 'gitlab-webhook:' || COALESCE((SELECT id::text FROM smoke_project LIMIT 1), '') || ':%'
)
DELETE FROM audit_log
WHERE entity_id IN (SELECT id FROM smoke_project)
   OR entity_id IN (SELECT id FROM smoke_jobs)
   OR payload->>'job_id' IN (SELECT id::text FROM smoke_jobs);

WITH smoke_project AS (
  SELECT id FROM projects WHERE slug = :'project_slug'
)
DELETE FROM jobs
WHERE payload->>'project_id' IN (SELECT id::text FROM smoke_project)
   OR dedupe_key LIKE :'project_slug' || '%'
   OR dedupe_key LIKE 'gitlab-webhook:' || COALESCE((SELECT id::text FROM smoke_project LIMIT 1), '') || ':%';

WITH smoke_invitations AS (
  SELECT id FROM user_invitations WHERE email LIKE :'email_like'
)
DELETE FROM jobs
WHERE dedupe_key IN (SELECT 'invite-email:' || id::text FROM smoke_invitations)
   OR payload->>'email' LIKE :'email_like';

DELETE FROM projects WHERE slug = :'project_slug';

DELETE FROM issue_permissions
WHERE subject_type = 'user'
  AND subject_id IN (SELECT id::text FROM users WHERE email LIKE :'email_like');

DELETE FROM project_permissions
WHERE subject_type = 'user'
  AND subject_id IN (SELECT id::text FROM users WHERE email LIKE :'email_like');

DELETE FROM user_sessions
WHERE user_id IN (SELECT id FROM users WHERE email LIKE :'email_like');

DELETE FROM user_invitations WHERE email LIKE :'email_like';

DELETE FROM users WHERE email LIKE :'email_like';
SQL

  rm -rf "$TMP_DIR"
  exit "$status"
}

trap cleanup EXIT

wait_for_health() {
  local status=""

  for _ in $(seq 1 60); do
    status="$(curl --silent --show-error --output /dev/null --write-out '%{http_code}' "${BASE_URL}/health" || true)"
    if [[ "$status" == "200" ]]; then
      return 0
    fi
    sleep 2
  done

  fail "health check did not become ready at ${BASE_URL}/health; last HTTP status=${status}"
}

wait_for_readiness() {
  local status=""
  local body_file="${TMP_DIR}/readiness.json"

  for _ in $(seq 1 60); do
    status="$(
      curl \
        --silent \
        --show-error \
        --output "$body_file" \
        --write-out '%{http_code}' \
        --header "Accept: application/json" \
        "${BASE_URL}/health/ready" || true
    )"
    if [[ "$status" == "200" ]] && jq -e '.status == "ok"' "$body_file" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done

  echo "--- readiness response body ---" >&2
  cat "$body_file" >&2 || true
  echo >&2
  fail "readiness check did not become ready at ${BASE_URL}/health/ready; last HTTP status=${status}"
}

check_metrics() {
  local status=""
  local body_file="${TMP_DIR}/metrics.txt"
  local body=""

  status="$(
    curl \
      --silent \
      --show-error \
      --output "$body_file" \
      --write-out '%{http_code}' \
      "${BASE_URL}/metrics"
  )"
  assert_status 200 "$status" "$body_file" metrics
  body="$(cat "$body_file")"
  [[ "$body" == *"issuehub_jobs_total{status=\"pending\"}"* ]] || fail "metrics missing pending jobs gauge"
  [[ "$body" == *"issuehub_workers_healthy"* ]] || fail "metrics missing healthy workers gauge"
  [[ "$body" == *"issuehub_db_up 1"* ]] || fail "metrics missing healthy DB gauge"
}

assert_status() {
  local expected="$1"
  local actual="$2"
  local body_file="$3"
  local label="$4"

  if [[ "$actual" != "$expected" ]]; then
    echo "Unexpected HTTP status for ${label}: expected ${expected}, got ${actual}" >&2
    echo "--- response body ---" >&2
    cat "$body_file" >&2 || true
    echo >&2
    exit 1
  fi
}

request_json() {
  local method="$1"
  local path="$2"
  local cookie_file="$3"
  local payload="$4"
  local expected="$5"
  local label="$6"
  local body_file="${TMP_DIR}/${label}.json"
  local status=""
  local args=(
    --silent
    --show-error
    --output "$body_file"
    --write-out '%{http_code}'
    --request "$method"
    --header "Accept: application/json"
    --header "Origin: ${ORIGIN}"
    --cookie "$cookie_file"
    --cookie-jar "$cookie_file"
    "${BASE_URL}${path}"
  )

  if [[ -n "$payload" ]]; then
    args+=(--header "Content-Type: application/json" --data "$payload")
  fi

  status="$(curl "${args[@]}")"
  assert_status "$expected" "$status" "$body_file" "$label"
  LAST_RESPONSE="$body_file"
}

login() {
  local email="$1"
  local password="$2"
  local cookie_file="$3"
  local label="$4"
  local payload=""

  payload="$(jq -cn --arg email "$email" --arg password "$password" '{email: $email, password: $password}')"
  request_json POST /api/v1/auth/login "$cookie_file" "$payload" 200 "$label"
  jq -e '.user.id and .user.email' "$LAST_RESPONSE" >/dev/null
}

upload_file() {
  local issue_id="$1"
  local cookie_file="$2"
  local source_file="$3"
  local expected="$4"
  local label="$5"
  upload_file_with_options "$issue_id" "$cookie_file" "$source_file" "$expected" "$label" "text/plain" "$(basename "$source_file")"
}

upload_file_with_options() {
  local issue_id="$1"
  local cookie_file="$2"
  local source_file="$3"
  local expected="$4"
  local label="$5"
  local content_type="$6"
  local upload_filename="$7"
  local body_file="${TMP_DIR}/${label}.json"
  local status=""

  status="$(
    curl \
      --silent \
      --show-error \
      --output "$body_file" \
      --write-out '%{http_code}' \
      --request POST \
      --header "Accept: application/json" \
      --header "Origin: ${ORIGIN}" \
      --cookie "$cookie_file" \
      --cookie-jar "$cookie_file" \
      --form "file=@${source_file};filename=${upload_filename};type=${content_type}" \
      "${BASE_URL}/api/v1/issues/${issue_id}/uploads"
  )"

  assert_status "$expected" "$status" "$body_file" "$label"
  LAST_RESPONSE="$body_file"
}

download_file() {
  local proxy_path="$1"
  local cookie_file="$2"
  local target_file="$3"
  local expected="$4"
  local label="$5"
  local body_file="${TMP_DIR}/${label}.body"
  local status=""

  status="$(
    curl \
      --silent \
      --show-error \
      --output "$body_file" \
      --write-out '%{http_code}' \
      --header "Origin: ${ORIGIN}" \
      --cookie "$cookie_file" \
      --cookie-jar "$cookie_file" \
      "${BASE_URL}${proxy_path}"
  )"

  assert_status "$expected" "$status" "$body_file" "$label"
  mv "$body_file" "$target_file"
}

create_user() {
  local email="$1"
  local full_name="$2"
  local is_admin="$3"

  psql_scalar \
    -v email="$email" \
    -v full_name="$full_name" \
    -v password="$SMOKE_PASSWORD" \
    -v is_admin="$is_admin" \
    <<'SQL'
INSERT INTO users (email, full_name, password_hash, is_admin, active)
VALUES (:'email', :'full_name', crypt(:'password', gen_salt('bf')), :'is_admin'::boolean, TRUE)
ON CONFLICT (email)
DO UPDATE SET
  full_name = EXCLUDED.full_name,
  password_hash = EXCLUDED.password_hash,
  is_admin = EXCLUDED.is_admin,
  active = TRUE,
  updated_at = NOW()
RETURNING id;
SQL
}

assert_jq() {
  local expression="$1"
  local file="$2"
  local label="$3"

  if ! jq -e "$expression" "$file" >/dev/null; then
    echo "Assertion failed for ${label}: ${expression}" >&2
    echo "--- response body ---" >&2
    cat "$file" >&2 || true
    echo >&2
    exit 1
  fi
}

wait_for_sql_value() {
  local label="$1"
  local expected="$2"
  local query="$3"

  local value=""
  for _ in $(seq 1 45); do
    value="$(psql_scalar <<<"$query")"
    if [[ "$value" == "$expected" ]]; then
      return 0
    fi
    sleep 2
  done

  fail "${label} did not reach ${expected}; last value=${value}"
}

if [[ "${SMOKE_START_STACK:-0}" == "1" ]]; then
  log "starting docker compose stack"
  docker compose up -d --build
fi

log "waiting for ${BASE_URL}"
wait_for_health
wait_for_readiness
check_metrics

if bool_enabled "$SECURITY_SMOKE"; then
  log "checking SECRET_ENCRYPTION_KEY startup guard"
  set +e
  secret_guard_output="$(
    timeout 20 docker compose run --rm --no-deps \
      -e SECRET_ENCRYPTION_KEY= \
      -e REQUIRE_SECRET_ENCRYPTION_KEY=true \
      -e API_PORT=18080 \
      api 2>&1
  )"
  secret_guard_status=$?
  set -e
  if [[ "$secret_guard_status" -eq 0 || "$secret_guard_status" -eq 124 ]]; then
    echo "$secret_guard_output" >&2
    fail "SECRET_ENCRYPTION_KEY startup guard did not fail fast"
  fi
  if [[ "$secret_guard_output" != *"SECRET_ENCRYPTION_KEY"* ]]; then
    echo "$secret_guard_output" >&2
    fail "SECRET_ENCRYPTION_KEY startup guard failed without a clear error"
  fi
fi

log "creating smoke users"
ADMIN_USER_ID="$(create_user "$ADMIN_EMAIL" "Phase 3 Smoke Admin" true)"
RATE_USER_ID="$(create_user "$RATE_EMAIL" "Phase 3 Rate User" false)"
VIEW_USER_ID="$(create_user "$VIEW_EMAIL" "Phase 3 View User" false)"
CREATE_USER_ID="$(create_user "$CREATE_EMAIL" "Phase 3 Create User" false)"
READ_USER_ID="$(create_user "$READ_EMAIL" "Phase 3 Read User" false)"
COMMENT_USER_ID="$(create_user "$COMMENT_EMAIL" "Phase 3 Comment User" false)"
EDIT_USER_ID="$(create_user "$EDIT_EMAIL" "Phase 3 Edit User" false)"
ISSUE_ADMIN_USER_ID="$(create_user "$ISSUE_ADMIN_EMAIL" "Phase 3 Issue Admin User" false)"
PROJECT_ADMIN_USER_ID="$(create_user "$PROJECT_ADMIN_EMAIL" "Phase 3 Project Admin User" false)"

if bool_enabled "$SECURITY_SMOKE"; then
  log "checking auth and password recovery rate limits"
  bad_login_payload="$(jq -cn --arg email "$RATE_EMAIL" '{email: $email, password: "wrong-password"}')"
  request_json POST /api/v1/auth/login "$RATE_COOKIE" "$bad_login_payload" 401 rate-login-1
  request_json POST /api/v1/auth/login "$RATE_COOKIE" "$bad_login_payload" 401 rate-login-2
  request_json POST /api/v1/auth/login "$RATE_COOKIE" "$bad_login_payload" 429 rate-login-limited

  recovery_payload="$(jq -cn --arg email "$RATE_EMAIL" '{email: $email}')"
  request_json POST /api/v1/auth/password-recovery "$RATE_COOKIE" "$recovery_payload" 204 rate-recovery-1
  request_json POST /api/v1/auth/password-recovery "$RATE_COOKIE" "$recovery_payload" 204 rate-recovery-2
  request_json POST /api/v1/auth/password-recovery "$RATE_COOKIE" "$recovery_payload" 429 rate-recovery-limited
fi

log "logging in as smoke admin"
login "$ADMIN_EMAIL" "$SMOKE_PASSWORD" "$ADMIN_COOKIE" admin-login
request_json GET /api/v1/admin/health "$ADMIN_COOKIE" "" 200 admin-health
assert_jq '.status == "ok" and (.workers | length) >= 1 and .queue.pending_jobs >= 0' "$LAST_RESPONSE" "admin health overview"

if bool_enabled "$SECURITY_SMOKE"; then
  log "checking invitation resend rate limit"
  invitation_payload="$(jq -cn --arg email "${SMOKE_PREFIX}-invite@example.invalid" '{email: $email, is_admin: false}')"
  request_json POST /api/v1/users/invitations "$ADMIN_COOKIE" "$invitation_payload" 201 rate-create-invitation
  INVITATION_ID="$(jq -r '.id' "$LAST_RESPONSE")"
  request_json POST "/api/v1/users/invitations/${INVITATION_ID}/resend" "$ADMIN_COOKIE" "" 200 rate-resend-1
  request_json POST "/api/v1/users/invitations/${INVITATION_ID}/resend" "$ADMIN_COOKIE" "" 200 rate-resend-2
  request_json POST "/api/v1/users/invitations/${INVITATION_ID}/resend" "$ADMIN_COOKIE" "" 429 rate-resend-limited
fi

log "creating local-first project and issues"
project_payload="$(jq -cn \
  --arg slug "$PROJECT_SLUG" \
  --arg name "Phase 3 Smoke ${RUN_ID}" \
  '{slug: $slug, name: $name, description: "Automated phase 3 smoke project"}')"
request_json POST /api/v1/projects "$ADMIN_COOKIE" "$project_payload" 201 create-project
PROJECT_ID="$(jq -r '.id' "$LAST_RESPONSE")"

issue_a_payload="$(jq -cn '{title: "Phase 3 Issue A", description: "Local-first issue A"}')"
request_json POST "/api/v1/projects/${PROJECT_ID}/issues" "$ADMIN_COOKIE" "$issue_a_payload" 201 create-issue-a
ISSUE_A_ID="$(jq -r '.id' "$LAST_RESPONSE")"

issue_b_payload="$(jq -cn '{title: "Phase 3 Issue B", description: "Local-first issue B"}')"
request_json POST "/api/v1/projects/${PROJECT_ID}/issues" "$ADMIN_COOKIE" "$issue_b_payload" 201 create-issue-b
ISSUE_B_ID="$(jq -r '.id' "$LAST_RESPONSE")"

log "checking internal 500 error redaction"
psql_exec \
  -v project_id="$PROJECT_ID" \
  -v gitlab_project_id="$GITLAB_PROJECT_ID" \
  <<'SQL' >/dev/null
INSERT INTO project_gitlab_integrations (
  project_id,
  gitlab_base_url,
  gitlab_api_base_url,
  gitlab_project_id,
  token,
  webhook_secret,
  token_encrypted,
  webhook_secret_encrypted,
  verify_tls,
  sync_enabled
)
VALUES (
  :'project_id'::uuid,
  'https://gitlab.example.invalid',
  'https://gitlab.example.invalid/api/v4',
  :'gitlab_project_id'::bigint,
  '',
  '',
  'not-base64',
  'not-base64',
  FALSE,
  FALSE
)
ON CONFLICT (project_id)
DO UPDATE SET
  gitlab_base_url = EXCLUDED.gitlab_base_url,
  gitlab_api_base_url = EXCLUDED.gitlab_api_base_url,
  gitlab_project_id = EXCLUDED.gitlab_project_id,
  token = EXCLUDED.token,
  webhook_secret = EXCLUDED.webhook_secret,
  token_encrypted = EXCLUDED.token_encrypted,
  webhook_secret_encrypted = EXCLUDED.webhook_secret_encrypted,
  verify_tls = EXCLUDED.verify_tls,
  sync_enabled = EXCLUDED.sync_enabled,
  updated_at = NOW();
SQL

internal_error_body="${TMP_DIR}/internal-error.json"
internal_error_payload="$(jq -cn \
  --argjson gitlab_project_id "$GITLAB_PROJECT_ID" \
  '{object_kind: "issue", project: {id: $gitlab_project_id}, object_attributes: {iid: 99120}}')"
internal_error_status="$(
  curl \
    --silent \
    --show-error \
    --output "$internal_error_body" \
    --write-out '%{http_code}' \
    --request POST \
    --header "Accept: application/json" \
    --header "Content-Type: application/json" \
    --header "X-Gitlab-Event: Issue Hook" \
    --header "X-Gitlab-Token: smoke-token" \
    --data "$internal_error_payload" \
    "${BASE_URL}/api/v1/gitlab/webhooks/${PROJECT_ID}"
)"
assert_status 500 "$internal_error_status" "$internal_error_body" internal-error-redaction
if bool_enabled "$(env_value DEBUG false)"; then
  assert_jq '.message | contains("failed to decode encrypted secret")' "$internal_error_body" "debug 500 detail"
else
  assert_jq '.message == "Internal server error"' "$internal_error_body" "masked 500 detail"
fi

psql_exec -v project_id="$PROJECT_ID" <<'SQL' >/dev/null
DELETE FROM project_gitlab_integrations
WHERE project_id = :'project_id'::uuid;
SQL

log "checking upload filename and content-type hardening"
SANITIZED_SOURCE="${TMP_DIR}/sanitized.txt"
printf 'sanitized upload %s\n' "$RUN_ID" > "$SANITIZED_SOURCE"
upload_file_with_options "$ISSUE_A_ID" "$ADMIN_COOKIE" "$SANITIZED_SOURCE" 201 upload-sanitized-filename "text/plain" "../../phase3-smoke.txt"
assert_jq '.filename == "phase3-smoke.txt"' "$LAST_RESPONSE" "upload filename sanitized"

BLOCKED_SOURCE="${TMP_DIR}/blocked.exe"
printf 'blocked upload %s\n' "$RUN_ID" > "$BLOCKED_SOURCE"
upload_file_with_options "$ISSUE_A_ID" "$ADMIN_COOKIE" "$BLOCKED_SOURCE" 415 upload-blocked-content-type "application/x-msdownload" "blocked.exe"

if bool_enabled "$SECURITY_SMOKE"; then
  OVERSIZED_SOURCE="${TMP_DIR}/oversized.txt"
  head -c "$(( $(env_value UPLOAD_MAX_BYTES 64) + 1 ))" /dev/zero > "$OVERSIZED_SOURCE"
  upload_file_with_options "$ISSUE_A_ID" "$ADMIN_COOKIE" "$OVERSIZED_SOURCE" 413 upload-too-large "text/plain" "oversized.txt"
fi

log "checking local attachment upload, comment persistence and download"
ATTACHMENT_SOURCE="${TMP_DIR}/${SMOKE_PREFIX}-attachment.txt"
printf 'phase 3 smoke attachment %s\n' "$RUN_ID" > "$ATTACHMENT_SOURCE"
upload_file "$ISSUE_A_ID" "$ADMIN_COOKIE" "$ATTACHMENT_SOURCE" 201 upload-attachment
UPLOAD_MARKDOWN="$(jq -r '.markdown' "$LAST_RESPONSE")"

if bool_enabled "$SECURITY_SMOKE"; then
  EXTRA_UPLOAD_SOURCE="${TMP_DIR}/extra-upload.txt"
  printf 'extra upload %s\n' "$RUN_ID" > "$EXTRA_UPLOAD_SOURCE"
  upload_file "$ISSUE_A_ID" "$ADMIN_COOKIE" "$EXTRA_UPLOAD_SOURCE" 201 upload-rate-last-allowed
  upload_file "$ISSUE_A_ID" "$ADMIN_COOKIE" "$EXTRA_UPLOAD_SOURCE" 429 upload-rate-limited
fi

comment_payload="$(jq -cn \
  --arg body "Admin comment with local attachment ${UPLOAD_MARKDOWN}" \
  '{body: $body, reply_to_note_id: null}')"
request_json POST "/api/v1/issues/${ISSUE_A_ID}/comments" "$ADMIN_COOKIE" "$comment_payload" 201 admin-comment-with-attachment
assert_jq '.attachments | length == 1' "$LAST_RESPONSE" "comment attachment persisted"
ATTACHMENT_PROXY_PATH="$(jq -r '.attachments[0].proxy_path' "$LAST_RESPONSE")"

log "granting project and issue access through API"
project_access_payload="$(jq -cn \
  --arg view_user "$VIEW_USER_ID" \
  --arg create_user "$CREATE_USER_ID" \
  --arg project_admin_user "$PROJECT_ADMIN_USER_ID" \
  '{
    assignments: [
      {subject_type: "user", subject_id: $view_user, permission: "view"},
      {subject_type: "user", subject_id: $create_user, permission: "create_issue"},
      {subject_type: "user", subject_id: $project_admin_user, permission: "admin"}
    ]
  }')"
request_json PUT "/api/v1/projects/${PROJECT_ID}/access" "$ADMIN_COOKIE" "$project_access_payload" 200 update-project-access

issue_access_payload="$(jq -cn \
  --arg read_user "$READ_USER_ID" \
  --arg comment_user "$COMMENT_USER_ID" \
  --arg edit_user "$EDIT_USER_ID" \
  --arg issue_admin_user "$ISSUE_ADMIN_USER_ID" \
  '{
    assignments: [
      {user_id: $read_user, permission: "read"},
      {user_id: $comment_user, permission: "comment"},
      {user_id: $edit_user, permission: "edit"},
      {user_id: $issue_admin_user, permission: "admin"}
    ]
  }')"
request_json PUT "/api/v1/issues/${ISSUE_A_ID}/access" "$ADMIN_COOKIE" "$issue_access_payload" 200 update-issue-access

log "checking project view role"
login "$VIEW_EMAIL" "$SMOKE_PASSWORD" "$VIEW_COOKIE" view-login
request_json GET /api/v1/issues "$VIEW_COOKIE" "" 200 view-list-issues
assert_jq "any(.[]; .id == \"${ISSUE_A_ID}\")" "$LAST_RESPONSE" "view user sees issue A"
assert_jq "any(.[]; .id == \"${ISSUE_B_ID}\")" "$LAST_RESPONSE" "view user sees issue B"
request_json GET "/api/v1/issues/${ISSUE_A_ID}" "$VIEW_COOKIE" "" 200 view-issue-detail
assert_jq '.issue.capabilities.can_comment == false and .issue.capabilities.can_edit == false' "$LAST_RESPONSE" "view is read-only"
view_comment_payload="$(jq -cn '{body: "view user should not comment", reply_to_note_id: null}')"
request_json POST "/api/v1/issues/${ISSUE_A_ID}/comments" "$VIEW_COOKIE" "$view_comment_payload" 404 view-comment-forbidden
view_create_payload="$(jq -cn '{title: "view user should not create", description: ""}')"
request_json POST "/api/v1/projects/${PROJECT_ID}/issues" "$VIEW_COOKIE" "$view_create_payload" 403 view-create-forbidden

log "checking project create_issue role without full visibility"
login "$CREATE_EMAIL" "$SMOKE_PASSWORD" "$CREATE_COOKIE" create-login
request_json GET /api/v1/projects "$CREATE_COOKIE" "" 200 create-list-projects
assert_jq "any(.[]; .id == \"${PROJECT_ID}\" and .capabilities.can_create_issue == true and .capabilities.can_manage == false)" "$LAST_RESPONSE" "create_issue project capability"
request_json GET /api/v1/issues "$CREATE_COOKIE" "" 200 create-list-issues-before-own
assert_jq "all(.[]; .id != \"${ISSUE_A_ID}\")" "$LAST_RESPONSE" "create_issue user cannot see issue A"
assert_jq "all(.[]; .id != \"${ISSUE_B_ID}\")" "$LAST_RESPONSE" "create_issue user cannot see issue B"
own_issue_payload="$(jq -cn '{title: "Phase 3 Own Issue", description: "created by create_issue-only user"}')"
request_json POST "/api/v1/projects/${PROJECT_ID}/issues" "$CREATE_COOKIE" "$own_issue_payload" 201 create-own-issue
OWN_ISSUE_ID="$(jq -r '.id' "$LAST_RESPONSE")"
assert_jq '.capabilities.can_manage_access == true and .capabilities.can_change_state == true' "$LAST_RESPONSE" "creator gets issue admin"
request_json GET /api/v1/issues "$CREATE_COOKIE" "" 200 create-list-issues-after-own
assert_jq "any(.[]; .id == \"${OWN_ISSUE_ID}\")" "$LAST_RESPONSE" "create_issue user sees own issue"
assert_jq "all(.[]; .id != \"${ISSUE_A_ID}\")" "$LAST_RESPONSE" "create_issue user still cannot see issue A"

log "checking issue read role"
login "$READ_EMAIL" "$SMOKE_PASSWORD" "$READ_COOKIE" read-login
request_json GET "/api/v1/issues/${ISSUE_A_ID}" "$READ_COOKIE" "" 200 read-detail
assert_jq '.issue.capabilities.can_view == true and .issue.capabilities.can_comment == false and .issue.capabilities.can_edit == false' "$LAST_RESPONSE" "read capabilities"
read_comment_payload="$(jq -cn '{body: "read user should not comment", reply_to_note_id: null}')"
request_json POST "/api/v1/issues/${ISSUE_A_ID}/comments" "$READ_COOKIE" "$read_comment_payload" 404 read-comment-forbidden
read_patch_payload="$(jq -cn '{title: "read user should not edit"}')"
request_json PATCH "/api/v1/issues/${ISSUE_A_ID}" "$READ_COOKIE" "$read_patch_payload" 403 read-edit-forbidden

log "checking issue comment role"
login "$COMMENT_EMAIL" "$SMOKE_PASSWORD" "$COMMENT_COOKIE" comment-login
request_json GET "/api/v1/issues/${ISSUE_A_ID}" "$COMMENT_COOKIE" "" 200 comment-detail
assert_jq '.issue.capabilities.can_comment == true and .issue.capabilities.can_change_state == false' "$LAST_RESPONSE" "comment capabilities"
comment_payload="$(jq -cn '{body: "comment-only user can comment", reply_to_note_id: null}')"
request_json POST "/api/v1/issues/${ISSUE_A_ID}/comments" "$COMMENT_COOKIE" "$comment_payload" 201 comment-role-comment
comment_close_payload="$(jq -cn '{state: "closed"}')"
request_json PATCH "/api/v1/issues/${ISSUE_A_ID}" "$COMMENT_COOKIE" "$comment_close_payload" 403 comment-close-forbidden
comment_create_payload="$(jq -cn '{title: "comment user should not create", description: ""}')"
request_json POST "/api/v1/projects/${PROJECT_ID}/issues" "$COMMENT_COOKIE" "$comment_create_payload" 403 comment-create-forbidden
request_json GET /api/v1/jobs "$COMMENT_COOKIE" "" 403 jobs-forbidden-to-non-admin

COMMENT_DOWNLOAD="${TMP_DIR}/comment-download.txt"
download_file "$ATTACHMENT_PROXY_PATH" "$COMMENT_COOKIE" "$COMMENT_DOWNLOAD" 200 comment-download-attachment
cmp "$ATTACHMENT_SOURCE" "$COMMENT_DOWNLOAD" >/dev/null || fail "downloaded attachment did not match uploaded content"

log "checking issue edit and issue admin roles"
login "$EDIT_EMAIL" "$SMOKE_PASSWORD" "$EDIT_COOKIE" edit-login
edit_payload="$(jq -cn '{title: "Phase 3 Issue A Edited", description: "edited by issue edit role", state: "closed"}')"
request_json PATCH "/api/v1/issues/${ISSUE_A_ID}" "$EDIT_COOKIE" "$edit_payload" 200 edit-issue
assert_jq '.state == "closed" and .capabilities.can_edit == true and .capabilities.can_manage_access == false' "$LAST_RESPONSE" "edit role can edit but not manage"
reopen_payload="$(jq -cn '{state: "open"}')"
request_json PATCH "/api/v1/issues/${ISSUE_A_ID}" "$EDIT_COOKIE" "$reopen_payload" 200 edit-reopen-issue

login "$ISSUE_ADMIN_EMAIL" "$SMOKE_PASSWORD" "$ISSUE_ADMIN_COOKIE" issue-admin-login
request_json GET "/api/v1/issues/${ISSUE_A_ID}/access" "$ISSUE_ADMIN_COOKIE" "" 200 issue-admin-access
assert_jq '.assignments | length >= 4' "$LAST_RESPONSE" "issue admin can inspect access"

log "checking project admin role"
login "$PROJECT_ADMIN_EMAIL" "$SMOKE_PASSWORD" "$PROJECT_ADMIN_COOKIE" project-admin-login
request_json GET "/api/v1/projects/${PROJECT_ID}/access" "$PROJECT_ADMIN_COOKIE" "" 200 project-admin-access
project_admin_issue_payload="$(jq -cn '{title: "Phase 3 Project Admin Issue", description: "created by project admin"}')"
request_json POST "/api/v1/projects/${PROJECT_ID}/issues" "$PROJECT_ADMIN_COOKIE" "$project_admin_issue_payload" 201 project-admin-create-issue
project_admin_close_payload="$(jq -cn '{state: "closed"}')"
request_json PATCH "/api/v1/issues/${ISSUE_B_ID}" "$PROJECT_ADMIN_COOKIE" "$project_admin_close_payload" 200 project-admin-close-issue

log "checking GitLab-disabled core workflow"
request_json GET "/api/v1/issues/${ISSUE_A_ID}" "$ADMIN_COOKIE" "" 200 admin-local-detail
assert_jq '.issue.gitlab_issue_iid == 0 and .issue.sync_state == "local"' "$LAST_RESPONSE" "local issue has no GitLab dependency"

log "checking webhook enqueue, job list, worker skip and dead/stale queue handling"
integration_payload="$(jq -cn \
  --arg webhook_secret "$WEBHOOK_SECRET" \
  --argjson gitlab_project_id "$GITLAB_PROJECT_ID" \
  '{
    gitlab_base_url: "https://gitlab.example.invalid",
    gitlab_api_base_url: "https://gitlab.example.invalid/api/v4",
    gitlab_project_id: $gitlab_project_id,
    token: "phase3-smoke-token",
    webhook_secret: $webhook_secret,
    verify_tls: false,
    sync_enabled: false
  }')"
request_json POST "/api/v1/projects/${PROJECT_ID}/gitlab-integration" "$ADMIN_COOKIE" "$integration_payload" 200 upsert-disabled-integration

webhook_body="${TMP_DIR}/webhook.json"
webhook_payload="$(jq -cn \
  --argjson gitlab_project_id "$GITLAB_PROJECT_ID" \
  '{object_kind: "issue", project: {id: $gitlab_project_id}, object_attributes: {iid: 99123}}')"
webhook_status="$(
  curl \
    --silent \
    --show-error \
    --output "$webhook_body" \
    --write-out '%{http_code}' \
    --request POST \
    --header "Accept: application/json" \
    --header "Content-Type: application/json" \
    --header "X-Gitlab-Event: Issue Hook" \
    --header "X-Gitlab-Token: ${WEBHOOK_SECRET}" \
    --data "$webhook_payload" \
    "${BASE_URL}/api/v1/gitlab/webhooks/${PROJECT_ID}"
)"
assert_status 202 "$webhook_status" "$webhook_body" webhook-enqueue
WEBHOOK_JOB_ID="$(jq -r '.job_id' "$webhook_body")"
[[ "$WEBHOOK_JOB_ID" != "null" && -n "$WEBHOOK_JOB_ID" ]] || fail "webhook response did not include job_id"

request_json GET /api/v1/jobs "$ADMIN_COOKIE" "" 200 admin-list-jobs
assert_jq "any(.[]; .id == \"${WEBHOOK_JOB_ID}\")" "$LAST_RESPONSE" "admin jobs list includes webhook job"

wait_for_sql_value "webhook job status" done \
  "SELECT status FROM jobs WHERE id = '${WEBHOOK_JOB_ID}'::uuid;"

wait_for_sql_value "webhook skipped audit" 1 \
  "SELECT COUNT(*) FROM audit_log WHERE entity_id = '${PROJECT_ID}'::uuid AND action = 'gitlab.webhook.skipped' AND payload->>'job_id' = '${WEBHOOK_JOB_ID}';"

DEAD_JOB_ID="$(
  psql_scalar \
    -v dedupe_key="${SMOKE_PREFIX}-dead-job" \
    <<'SQL'
INSERT INTO jobs (topic, payload, status, available_at, attempt_count, dedupe_key, created_at, updated_at)
VALUES ('gitlab.webhook.received', '{}'::jsonb, 'pending', NOW(), 4, :'dedupe_key', NOW() - INTERVAL '100 years', NOW())
ON CONFLICT (dedupe_key)
DO UPDATE SET status = 'pending', payload = '{}'::jsonb, attempt_count = 4, available_at = NOW(), locked_at = NULL, locked_by = NULL, last_error = NULL, created_at = NOW() - INTERVAL '100 years', updated_at = NOW()
RETURNING id;
SQL
)"
wait_for_sql_value "dead job status" dead \
  "SELECT status FROM jobs WHERE id = '${DEAD_JOB_ID}'::uuid;"
wait_for_sql_value "dead job audit" 1 \
  "SELECT COUNT(*) FROM audit_log WHERE entity_id = '${DEAD_JOB_ID}'::uuid AND action = 'dead';"

STALE_JOB_ID="$(
  psql_scalar \
    -v dedupe_key="${SMOKE_PREFIX}-stale-job" \
    -v run_id="$RUN_ID" \
    <<'SQL'
INSERT INTO jobs (topic, payload, status, available_at, attempt_count, locked_at, locked_by, dedupe_key, created_at, updated_at)
VALUES (
  'phase3.smoke.noop',
  jsonb_build_object('run_id', :'run_id'),
  'processing',
  NOW(),
  0,
  NOW() - INTERVAL '10 minutes',
  'phase3-smoke',
  :'dedupe_key',
  NOW() - INTERVAL '100 years',
  NOW()
)
ON CONFLICT (dedupe_key)
DO UPDATE SET status = 'processing', payload = EXCLUDED.payload, attempt_count = 0, available_at = NOW(), locked_at = NOW() - INTERVAL '10 minutes', locked_by = 'phase3-smoke', last_error = NULL, created_at = NOW() - INTERVAL '100 years', updated_at = NOW()
RETURNING id;
SQL
)"
wait_for_sql_value "stale job recovery audit" 1 \
  "SELECT COUNT(*) FROM audit_log WHERE entity_id = '${STALE_JOB_ID}'::uuid AND action = 'stale_lock_recovered';"

log "phase 3 smoke passed"
