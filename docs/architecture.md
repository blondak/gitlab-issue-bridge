# Architektonický plán

## Cíl

Postavit systém, který:

- synchronizuje issues z GitLabu přes API a webhooky,
- umí obsluhovat více interních projektů, každý s vlastní GitLab integrací,
- umožní řídit přístupy na úrovni jednotlivých issue,
- udržuje konzistentní stav změn pomocí serializace a PostgreSQL queue,
- běží v Docker Compose jako sada menších služeb.

## Služby

### frontend

React aplikace pro:

- seznam issue,
- detail issue,
- správu ACL,
- audit a synchronizační stavy.

### api

Rust `Axum` služba, která:

- poskytuje REST API pro frontend,
- obsluhuje issue registry,
- řídí oprávnění,
- enqueueuje joby do PostgreSQL,
- funguje jako proxy vrstva pro comments a attachment download/upload.

### worker

Rust background worker, který:

- polluje PostgreSQL queue,
- zpracovává synchronizační a reconcile joby,
- drží serializaci změn na úrovni issue.

### postgres

Jedna PostgreSQL instance s oddělenými tabulkami pro:

- issues,
- permissions,
- jobs,
- audit log.

## Datový model

### `issues`

- interní reprezentace issue
- patří do interního projektu
- obsahuje mapování na GitLab `gitlab_issue_iid`
- drží `version`, `last_source`, `sync_state`

### `projects`

- interní projekt vytvářený administrátorem
- drží název, slug a aktivní stav

### `project_gitlab_integrations`

- projektová konfigurace pro GitLab
- `gitlab_base_url`
- `gitlab_api_base_url`
- `gitlab_project_id`
- `token`
- `webhook_secret`
- `verify_tls`
- `sync_enabled`

### `issue_permissions`

- ACL override na úrovni issue
- `subject_type`: `user`, `group`, `role`
- `permission`: `read`, `comment`, `edit`, `close`, `admin`
- `effect`: `allow`, `deny`

### `jobs`

- PostgreSQL queue
- `topic`, `payload`, `status`, `available_at`, `attempt_count`, `locked_at`, `locked_by`, `dedupe_key`

### `issue_attachments`

- Přílohy jsou interní entity IssueHubu.
- `storage_backend=local` znamená lokální autoritativní soubor pod attachments storage; tento soubor je součást produkčních záloh.
- `storage_backend=gitlab` znamená GitLab jako autoritativní zdroj; `storage_path` je pouze lokální cache v `ATTACHMENT_CACHE_DIR`.
- Pokud GitLab cache soubor chybí, API ho při downloadu znovu stáhne z GitLabu přes server-side proxy a uloží novou cache metadata.
- Browser nikdy nedostává privátní GitLab upload URL ani token.

### `audit_log`

- audit všech podstatných změn a synchronizačních operací

## Synchronizační model

### Inbound

1. GitLab webhook nebo pull sync vytvoří job.
2. Worker načte stav z GitLabu.
3. API/worker aktualizuje lokální issue.
4. Audit log uloží změnu.

### Outbound

1. Uživatel přes frontend změní issue nebo ACL.
2. API provede autorizaci.
3. API zapíše změnu a vytvoří outbox job v jedné transakci.
4. Worker pushne změnu do GitLabu.
5. Po potvrzení uloží nový sync stav.

## GitLab konfigurace na projektu

GitLab integrace je uložená per projekt v databázi. To umožní:

- více různých GitLab projektů v jedné aplikaci,
- odlišné tokeny a webhook secret pro každý projekt,
- zapínat nebo vypínat synchronizaci per projekt.

## MVP implementace

1. Infrastruktura a monorepo
2. PostgreSQL schéma
3. API health, list issues, enqueue job
4. Worker polling loop
5. Frontend dashboard
6. GitLab connector
7. ACL editor a audit pohled

## Důležité technické zásady

- serializace změn per `issue_id`
- `FOR UPDATE SKIP LOCKED` pro queue consumer
- outbox pattern v PostgreSQL
- idempotentní joby přes `dedupe_key`
- periodický reconciliation job
