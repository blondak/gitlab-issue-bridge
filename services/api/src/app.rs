use std::env;

use axum::{
    extract::Request,
    http::{header, HeaderValue, Method, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::Response,
    routing::{get, patch, post},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{
    error::ApiError,
    routes::{attachments, auth, docs, health, invitations, issues, jobs, projects, users, webhooks},
    state::AppState,
};

pub fn build_router(state: AppState) -> Router {
    let frontend_origin = env::var("FRONTEND_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let origin = HeaderValue::from_str(&frontend_origin)
        .unwrap_or_else(|_| HeaderValue::from_static("http://localhost:3000"));

    let protected_routes = Router::new()
        .route("/api/v1/overview", get(health::get_overview))
        .route("/api/v1/projects", get(projects::list_projects).post(projects::create_project))
        .route(
            "/api/v1/projects/{project_id}",
            patch(projects::update_project).delete(projects::delete_project),
        )
        .route(
            "/api/v1/projects/{project_id}/access",
            get(projects::get_project_access).put(projects::update_project_access),
        )
        .route(
            "/api/v1/projects/{project_id}/gitlab-integration",
            post(projects::upsert_gitlab_integration).delete(projects::delete_gitlab_integration),
        )
        .route(
            "/api/v1/projects/{project_id}/gitlab-integration/validate",
            post(projects::validate_gitlab_integration),
        )
        .route(
            "/api/v1/projects/{project_id}/gitlab-integration/import",
            post(projects::import_gitlab_issues),
        )
        .route("/api/v1/users", get(users::get_user_management_overview))
        .route("/api/v1/users/{user_id}", patch(users::update_user))
        .route(
            "/api/v1/users/{user_id}/access",
            get(users::get_user_access).put(users::update_user_access),
        )
        .route("/api/v1/users/invitations", post(users::create_invitation))
        .route(
            "/api/v1/users/invitations/{invitation_id}",
            axum::routing::delete(users::delete_invitation),
        )
        .route(
            "/api/v1/users/invitations/{invitation_id}/resend",
            post(users::resend_invitation),
        )
        .route("/api/v1/issues", get(issues::list_issues))
        .route("/api/v1/projects/{project_id}/issues", post(issues::create_issue))
        .route("/api/v1/projects/{project_id}/uploads", post(issues::upload_project_issue_attachment))
        .route(
            "/api/v1/issues/{issue_id}",
            get(issues::get_issue_detail).patch(issues::update_issue),
        )
        .route("/api/v1/issues/{issue_id}/gitlab-link", get(issues::redirect_issue_to_gitlab))
        .route(
            "/api/v1/issues/{issue_id}/comments/{note_id}/gitlab-link",
            get(issues::redirect_note_to_gitlab),
        )
        .route("/api/v1/issues/{issue_id}/comments", post(issues::create_issue_comment_handler))
        .route("/api/v1/issues/{issue_id}/comments/sync", post(issues::sync_issue_comments))
        .route("/api/v1/issues/{issue_id}/uploads", post(issues::upload_issue_attachment))
        .route("/api/v1/uploads/{upload_id}", axum::routing::delete(issues::delete_issue_upload))
        .route(
            "/api/v1/issues/{issue_id}/access",
            get(issues::get_issue_access).put(issues::update_issue_access),
        )
        .route(
            "/api/v1/issues/{issue_id}/access/{subject_type}/{subject_id}",
            axum::routing::delete(issues::remove_issue_permission),
        )
        .route("/api/v1/uploads/{upload_id}/download", get(issues::download_issue_upload))
        .route("/api/v1/auth/change-password", post(auth::change_password))
        .route(
            "/api/v1/attachments/{attachment_id}/download",
            get(attachments::download_attachment),
        )
        .route("/api/v1/admin/health", get(health::get_admin_health))
        .route("/api/v1/jobs", get(jobs::list_jobs).post(jobs::enqueue_job))
        .route_layer(from_fn_with_state(state.clone(), auth_middleware));

    Router::new()
        .route("/health", get(health::get_health))
        .route("/health/live", get(health::get_liveness))
        .route("/health/ready", get(health::get_readiness))
        .route("/metrics", get(health::get_metrics))
        .route("/api-docs", get(docs::api_docs))
        .route("/api-docs/openapi.json", get(docs::openapi_spec))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/me", get(auth::me).patch(auth::update_me))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/password-recovery", post(auth::request_password_recovery))
        .route(
            "/api/v1/auth/password-recovery/{token}",
            get(auth::get_password_recovery_preview),
        )
        .route(
            "/api/v1/auth/password-recovery/{token}/reset",
            post(auth::reset_password),
        )
        .route("/api/v1/invitations/{invite_token}", get(invitations::get_invitation_preview))
        .route(
            "/api/v1/invitations/{invite_token}/accept",
            post(invitations::accept_invitation),
        )
        .route(
            "/api/v1/gitlab/webhooks/{project_id}",
            post(webhooks::receive_gitlab_webhook),
        )
        .merge(protected_routes)
        .with_state(state.clone())
        .layer(from_fn_with_state(state.clone(), csrf_middleware))
        .layer(
            CorsLayer::new()
                .allow_origin(origin)
                .allow_credentials(true)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::PATCH,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        )
        .layer(TraceLayer::new_for_http())
}

async fn auth_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    auth::require_authenticated(axum::extract::State(state), headers)
        .await
        .map_err(|error| ApiError::new(StatusCode::UNAUTHORIZED, error.message))?;

    Ok(next.run(request).await)
}

async fn csrf_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if requires_csrf_check(&request) && !has_allowed_origin(&state, request.headers()) {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Invalid request origin"));
    }

    Ok(next.run(request).await)
}

fn requires_csrf_check(request: &Request) -> bool {
    if matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    ) {
        return false;
    }

    if request.uri().path().starts_with("/api/v1/gitlab/webhooks/") {
        return false;
    }

    request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .map(|cookies| cookies.split(';').any(|cookie| cookie.trim().starts_with("issuehub_session=")))
        .unwrap_or(false)
}

fn has_allowed_origin(state: &AppState, headers: &axum::http::HeaderMap) -> bool {
    let allowed_origins = [
        normalize_origin(&state.config.frontend_origin),
        normalize_origin(&state.config.public_frontend_url),
    ];

    if let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(normalize_origin)
    {
        return allowed_origins.iter().any(|allowed| allowed == &origin);
    }

    if let Some(referer) = headers
        .get(header::REFERER)
        .and_then(|value| value.to_str().ok())
    {
        let referer = referer.to_ascii_lowercase();
        return allowed_origins
            .iter()
            .any(|allowed| referer == *allowed || referer.starts_with(&format!("{allowed}/")));
    }

    false
}

fn normalize_origin(value: &str) -> String {
    value.trim().trim_end_matches('/').to_ascii_lowercase()
}
