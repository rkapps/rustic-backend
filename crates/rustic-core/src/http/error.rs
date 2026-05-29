use thiserror::Error;

/// Typed errors that can arise when making HTTP requests to external APIs.
///
/// Variants are mapped from HTTP status codes and transport-level failures in
/// [`HttpClient`](crate::http::HttpClient). Use [`HttpError::is_retryable`] to
/// decide whether a failed request is worth retrying.
#[derive(Error, Debug)]
pub enum HttpError {
    /// The server returned HTTP 429 — the caller has exceeded its request quota.
    #[error("Rate limited")]
    RateLimited,

    /// The server returned HTTP 503 — the upstream service is temporarily unavailable.
    #[error("Service unavailable")]
    ServiceUnavailable,

    /// The request did not complete within the allowed time.
    #[error("Request timeout")]
    Timeout,

    /// A transport-level failure occurred before a response was received (e.g. DNS, TCP).
    #[error("Network error: {0}")]
    NetworkError(String),

    /// The server returned HTTP 401 or 403 — credentials are missing or invalid.
    #[error("Authentication failed")]
    AuthenticationFailed,

    /// The API key could not be read or parsed from configuration.
    #[error("API Key parsing failed")]
    ApiKeyParsingFailed,

    /// The API version string could not be read or parsed from configuration.
    #[error("API Version parsing failed")]
    ApiVersionParsingFailed,

    /// The server returned HTTP 400 — the request payload was malformed or rejected.
    /// The inner string contains the raw error body from the server.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// The completion endpoint returned an application-level error.
    /// The inner string contains the provider's error message.
    #[error("Completion request error: {0}")]
    CompletionRequestError(String),

    /// The prompt or conversation history exceeded the model's context window.
    #[error("Context too long")]
    ContextTooLong,

    /// The agentic loop was stopped because it reached the configured tool-call iteration limit.
    #[error("Max tool iterations exceeded")]
    MaxIterationsExceeded,

    /// A catch-all for errors that do not map to a more specific variant.
    #[error("{0}")]
    Other(String),
}

impl HttpError {
    /// Returns `true` if the error is transient and the request may succeed on retry.
    ///
    /// The retryable variants are: [`RateLimited`](Self::RateLimited),
    /// [`ServiceUnavailable`](Self::ServiceUnavailable), [`Timeout`](Self::Timeout),
    /// and [`NetworkError`](Self::NetworkError).
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            HttpError::RateLimited
                | HttpError::ServiceUnavailable
                | HttpError::Timeout
                | HttpError::NetworkError(_)
        )
    }
}
