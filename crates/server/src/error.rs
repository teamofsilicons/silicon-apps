use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
    pub hint: String,
    pub details: Value,
}
impl ApiError {
    pub fn new(
        status: StatusCode,
        code: &str,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
            hint: hint.into(),
            details: Value::Null,
        }
    }
    pub fn bad(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            message,
            "Check the API contract and correct the indicated field.",
        )
    }
    pub fn forbidden() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "author_required",
            "Only an app author can perform this action.",
            "Ask an existing author to invite you.",
        )
    }
    pub fn missing() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "The requested app or resource is unavailable.",
            "Check its ID and sign in if it is private.",
        )
    }
    pub fn auth() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Sign in to Silicon Accounts to continue.",
            "Run apps login and retry.",
        )
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "conflict",
            message,
            "Refresh the current state and retry with a new idempotency key.",
        )
    }
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency_unavailable",
            message,
            "Configure the required service integration and retry with the same idempotency key.",
        )
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status,Json(json!({"error":{"code":self.code,"message":self.message,"hint":self.hint,"details":self.details}}))).into_response()
    }
}
impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        eprintln!("database failure: {e}");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_error",
            "The operation could not be saved.",
            "Retry with the same idempotency key.",
        )
    }
}
pub type Result<T> = std::result::Result<T, ApiError>;
