use std::sync::atomic::{AtomicBool, Ordering};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

pub type ApiResult<T> = Result<T, ApiError>;

static DEBUG_ERROR_RESPONSES: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

pub fn set_debug_error_responses(enabled: bool) {
    DEBUG_ERROR_RESPONSES.store(enabled, Ordering::Relaxed);
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let debug_error_responses = DEBUG_ERROR_RESPONSES.load(Ordering::Relaxed);
        let message = if self.status == StatusCode::INTERNAL_SERVER_ERROR && !debug_error_responses {
            "Internal server error".to_string()
        } else {
            self.message.clone()
        };

        if self.status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(
                error.message = %self.message,
                response.message = %message,
                debug_error_responses,
                "internal API error"
            );
        }

        (
            self.status,
            Json(ErrorResponse { message }),
        )
            .into_response()
    }
}

impl From<(StatusCode, String)> for ApiError {
    fn from(value: (StatusCode, String)) -> Self {
        Self::new(value.0, value.1)
    }
}

impl From<(StatusCode, &str)> for ApiError {
    fn from(value: (StatusCode, &str)) -> Self {
        Self::new(value.0, value.1)
    }
}

pub fn internal_error(error: sqlx::Error) -> ApiError {
    ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
