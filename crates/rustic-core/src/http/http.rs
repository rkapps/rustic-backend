//! HTTP client utilities for making requests to external APIs.
//!
//! This module provides a generic HTTP client built on top of `reqwest`
//! with built-in error handling, header management, and response
//! deserialization.
//!
//! # Examples
//!
//! ```rust
//! use rustic_core::http::HttpClient;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let client = HttpClient::new()?;
//!     Ok(())
//! }
//! ```

use anyhow::Result;
use reqwest::{Client, header::HeaderMap};
use tracing::{debug, error, trace};

use crate::http::error::HttpError;

/// Convenience alias for results that carry an [`HttpError`] on failure.
pub type HttpResult<T> = std::result::Result<T, HttpError>;

/// Thin wrapper around a [`reqwest::Client`] that maps HTTP and transport
/// errors to [`HttpError`] variants and handles response deserialization.
#[derive(Debug, Clone)]
pub struct HttpClient {
    client: Client,
}

/// Deserialized response body paired with the raw response headers.
///
/// Callers that need to inspect headers (e.g. rate-limit metadata, pagination
/// cursors) should use the `*_with_headers` variants that return this type.
pub struct HttpResponse<T> {
    /// Deserialized response body.
    pub body: T,
    /// Raw HTTP response headers returned by the server.
    pub headers: HeaderMap,
}

impl HttpClient {
    /// Create a new [`HttpClient`] backed by a default [`reqwest::Client`].
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: Client::new(),
        })
    }

    /// Send an HTTP GET request and deserialize the JSON response body into `T`.
    ///
    /// # Arguments
    ///
    /// * `url`     - The endpoint URL.
    /// * `headers` - Optional additional request headers (e.g. auth tokens).
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails to send, the server returns a
    /// non-2xx status code, or the response body cannot be deserialized into `T`.
    ///
    /// > **Note:** this method returns `anyhow::Result` rather than [`HttpResult`];
    /// > errors are not mapped to typed [`HttpError`] variants.
    pub async fn get_request<T: serde::de::DeserializeOwned + Send>(
        &self,
        url: String,
        headers: Option<reqwest::header::HeaderMap>,
    ) -> Result<T> {
        trace!("→ GET {}", url);

        let mut request = self.client.get(url);

        if let Some(h) = headers {
            request = request.headers(h);
        }

        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await?;

        trace!("← {} {}", status, text);

        // handle errors before attempting deserialization
        if status.is_client_error() || status.is_server_error() {
            error!("HTTP {} error: {}", status, text);
            return Err(anyhow::anyhow!("HTTP {} error: {}", status, text));
        }

        // include raw body in deserialization errors
        serde_json::from_str::<T>(&text).map_err(|e| {
            anyhow::anyhow!(
                "Failed to deserialize response: {}\nStatus: {}\nBody: {}",
                e,
                status,
                text
            )
        })
    }

    /// Sends an HTTP POST request and deserializes the response.
    ///
    /// # Arguments
    ///
    /// * `url`     - The endpoint URL
    /// * `headers` - Optional additional headers
    /// * `body`    - The request body as JSON
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The request fails to send
    /// - The server returns a non-2xx status code
    /// - The response cannot be deserialized into `T`
    ///
    /// # Examples
    ///
    /// ```rust
    /// #[derive(Deserialize)]
    /// struct MyResponse { id: String }
    ///
    /// let response: MyResponse = client
    ///     .post_request(
    ///         "https://api.example.com/resource".to_string(),
    ///         None,
    ///         serde_json::json!({"key": "value"}),
    ///     )
    ///     .await?;
    /// ```    
    pub async fn post_request<T: serde::de::DeserializeOwned + Send>(
        &self,
        url: String,
        headers: Option<reqwest::header::HeaderMap>,
        body: serde_json::Value,
    ) -> HttpResult<T> {
        let response = self
            .post_request_with_headers::<T>(url, headers, body)
            .await?;
        Ok(response.body)
    }

    /// Send a POST request and return both the deserialized body and the response headers.
    ///
    /// Handles SSE (`text/event-stream`) responses by extracting the first `data:` line before
    /// deserializing, so callers receive a uniform `T` regardless of content type.
    ///
    /// # Errors
    ///
    /// Maps HTTP status codes to typed [`HttpError`] variants:
    /// `400` → `InvalidRequest`, `401`/`403` → `AuthenticationFailed`,
    /// `429` → `RateLimited`, `503` → `ServiceUnavailable`, others → `Other`.
    pub async fn post_request_with_headers<T: serde::de::DeserializeOwned + Send>(
        &self,
        url: String,
        headers: Option<reqwest::header::HeaderMap>,
        body: serde_json::Value,
    ) -> HttpResult<HttpResponse<T>> {
        debug!("Url: {}", url);
        let mut request = self.client.post(url);

        if let Some(h) = headers {
            request = request.headers(h);
        }
        trace!("Body: {:#?}", body);

        let response = request.json(&body).send().await.map_err(|e| {
            error!("Error {:?}", e);
            if e.is_timeout() {
                HttpError::Timeout
            } else if e.is_connect() {
                HttpError::NetworkError(e.to_string())
            } else {
                HttpError::Other(e.to_string())
            }
        })?;

        let status = response.status();
        let response_headers = response.headers().clone();

        match status.as_u16() {
            200..=299 => {
                let content_type = response_headers
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();

                let text = response
                    .text()
                    .await
                    .map_err(|e| HttpError::NetworkError(e.to_string()))?;

                trace!("Raw response: {:#?}", text);

                // strip SSE wrapper if needed
                let json_text = if content_type.contains("text/event-stream") {
                    text.lines()
                        .find_map(|line| line.strip_prefix("data: "))
                        .ok_or_else(|| {
                            HttpError::Other("No data field in SSE response".to_string())
                        })?
                        .to_string()
                } else {
                    text
                };

                let body: T = serde_json::from_str(&json_text)
                    .map_err(|e| HttpError::Other(format!("Deserialization failed: {}", e)))?;

                Ok(HttpResponse {
                    body,
                    headers: response_headers,
                })
            }
            400 => {
                let error_body = response
                    .text()
                    .await
                    .map_err(|e| HttpError::NetworkError(e.to_string()))?;
                error!("❌ Bad Request: {}", error_body);
                Err(HttpError::InvalidRequest(error_body))
            }
            401 | 403 => {
                error!("❌ Authentication failed");
                Err(HttpError::AuthenticationFailed)
            }
            429 => {
                error!("❌ Rate limited");
                Err(HttpError::RateLimited)
            }
            503 => {
                error!("❌ Service unavailable");
                Err(HttpError::ServiceUnavailable)
            }
            _ => {
                let error_body = response
                    .text()
                    .await
                    .map_err(|e| HttpError::NetworkError(e.to_string()))?;
                error!("❌ HTTP {}: {}", status, error_body);
                Err(HttpError::Other(format!("HTTP {}", status)))
            }
        }
    }

    /// Send a fire-and-forget POST request where the response body is ignored.
    ///
    /// Use this for webhook calls or notification endpoints that return no meaningful body.
    /// Only the HTTP status is checked; `200–299` resolves to `()`.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::InvalidRequest`] on `400`, or [`HttpError::Other`] for any other
    /// non-2xx status. Transport failures surface as [`HttpError::Timeout`] or
    /// [`HttpError::NetworkError`].
    pub async fn post_notification(
        &self,
        url: String,
        headers: Option<reqwest::header::HeaderMap>,
        body: serde_json::Value,
    ) -> HttpResult<()> {
        let mut request = self.client.post(url);

        if let Some(h) = headers {
            request = request.headers(h);
        }

        let response = request.json(&body).send().await.map_err(|e| {
            error!("Error {:?}", e);
            if e.is_timeout() {
                HttpError::Timeout
            } else if e.is_connect() {
                HttpError::NetworkError(e.to_string())
            } else {
                HttpError::Other(e.to_string())
            }
        })?;

        let status = response.status();

        match status.as_u16() {
            200..=299 => Ok(()), // empty body is fine
            400 => {
                let error_body = response
                    .text()
                    .await
                    .map_err(|e| HttpError::NetworkError(e.to_string()))?;
                error!("❌ Bad Request: {}", error_body);
                Err(HttpError::InvalidRequest(error_body))
            }
            _ => {
                let error_body = response
                    .text()
                    .await
                    .map_err(|e| HttpError::NetworkError(e.to_string()))?;
                error!("❌ HTTP {}: {}", status, error_body);
                Err(HttpError::Other(format!("HTTP {}", status)))
            }
        }
    }

    /// Send a POST request and return the raw [`reqwest::Response`] for streaming.
    ///
    /// The response body is left unconsumed so the caller can drive the byte stream directly
    /// (e.g. for server-sent events). Status-code mapping is identical to
    /// [`post_request_with_headers`](Self::post_request_with_headers).
    pub async fn post_stream_request(
        &self,
        url: String,
        headers: Option<reqwest::header::HeaderMap>,
        body: serde_json::Value,
    ) -> HttpResult<reqwest::Response> {
        debug!("Url: {}", url);
        let mut request = self.client.post(url);

        if let Some(h) = headers {
            request = request.headers(h);
        }

        trace!("Body: {:#?}", &body);
        let response = request.json(&body).send().await.map_err(|e| {
            error!("Error {:?}", e);

            // Map reqwest errors to HttpError
            if e.is_timeout() {
                HttpError::Timeout
            } else if e.is_connect() {
                HttpError::NetworkError(e.to_string())
            } else {
                HttpError::Other(e.to_string())
            }
        })?;
        trace!("Raw response: {:#?}", response);

        // Save status before consuming response
        let status = response.status();

        match response.status().as_u16() {
            200..=299 => Ok(response),
            400 => {
                let error_body = response
                    .text()
                    .await
                    .map_err(|e| HttpError::NetworkError(e.to_string()))?;
                error!("❌ Bad Request: {}", error_body);
                Err(HttpError::InvalidRequest(error_body))
            }
            401 | 403 => {
                error!("❌ Authentication failed");
                Err(HttpError::AuthenticationFailed)
            }
            429 => {
                error!("❌ Rate limited");
                Err(HttpError::RateLimited)
            }
            503 => {
                error!("❌ Service unavailable");
                Err(HttpError::ServiceUnavailable)
            }
            _ => {
                let error_body = response
                    .text()
                    .await
                    .map_err(|e| HttpError::NetworkError(e.to_string()))?;
                error!("❌ HTTP {}: {}", status, error_body);
                Err(HttpError::Other(format!("HTTP {}", status)))
            }
        }
    }
}
