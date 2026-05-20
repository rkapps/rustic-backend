# rustic-core

Shared HTTP client, error types, and utilities used across the rustic-ai workspace.

## Contents

### `http`

`HttpClient` wraps `reqwest` and provides four methods, all of which map HTTP and transport errors to typed `HttpError` variants:

| Method | Use case |
|---|---|
| `post_request::<T>` | POST, deserialise body into `T` |
| `post_request_with_headers::<T>` | POST, deserialise body and return response headers |
| `post_notification` | POST fire-and-forget (response body ignored) |
| `post_stream_request` | POST, return raw `reqwest::Response` for SSE streaming |

`post_request_with_headers` transparently handles `text/event-stream` responses by extracting the first `data:` line before deserialisation.

### `error`

`HttpError` is a typed error enum covering all failure modes:

| Variant | Trigger |
|---|---|
| `RateLimited` | HTTP 429 |
| `ServiceUnavailable` | HTTP 503 |
| `AuthenticationFailed` | HTTP 401 / 403 |
| `InvalidRequest(String)` | HTTP 400 |
| `Timeout` | Request timeout |
| `NetworkError(String)` | Transport failure (DNS, TCP) |
| `ApiKeyParsingFailed` | Bad API key format |
| `ApiVersionParsingFailed` | Bad API version header |
| `CompletionRequestError(String)` | Provider-level completion error |
| `ContextTooLong` | Model context window exceeded |
| `MaxIterationsExceeded` | Agentic loop iteration cap hit |
| `Other(String)` | Uncategorised |

`HttpError::is_retryable()` returns `true` for `RateLimited`, `ServiceUnavailable`, `Timeout`, and `NetworkError`.

`HttpResult<T>` is a type alias for `Result<T, HttpError>`.

## Usage

```rust
use rustic_core::http::{HttpClient, HttpResult};
use rustic_core::error::HttpError;

let client = HttpClient::new()?;

// Blocking POST
let response: MyType = client
    .post_request(url, Some(headers), body)
    .await?;

// Streaming POST — consume the bytes_stream() yourself
let raw = client
    .post_stream_request(url, Some(headers), body)
    .await?;

// Retry logic
if err.is_retryable() {
    // back off and retry
}
```
