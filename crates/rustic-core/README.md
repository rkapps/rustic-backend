# rustic-core

Shared HTTP client, error types, `Tool` trait, and logging utilities used across the rustic-ai workspace. Every other crate depends on this one.

## Contents

### `agents::Tool`

The trait all agent-callable tools must implement:

```rust
#[async_trait]
pub trait Tool: Send + Sync + Debug {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn parameters(&self) -> serde_json::Value;  // JSON Schema object
    async fn execute(&self, value: serde_json::Value) -> Result<serde_json::Value>;
}
```

`rustic-agent` discovers tools via `ToolRegistry` and forwards `name`, `description`, and `parameters` to the LLM as tool definitions. When the model requests a call, the agent dispatches it to `execute`.

### `http`

`HttpClient` wraps `reqwest` with typed error mapping:

| Method | Use case |
|--------|----------|
| `post_request::<T>` | POST, deserialise response body into `T` |
| `post_request_with_headers::<T>` | POST, deserialise body and return response headers |
| `post_notification` | POST fire-and-forget (response body is discarded) |
| `post_stream_request` | POST, return the raw `reqwest::Response` for SSE streaming |

`post_request_with_headers` transparently handles `text/event-stream` responses by
extracting the first `data:` line before deserialisation.

### `http::HttpError`

Typed error enum:

| Variant | Trigger |
|---------|---------|
| `RateLimited` | HTTP 429 |
| `ServiceUnavailable` | HTTP 503 |
| `AuthenticationFailed` | HTTP 401 / 403 |
| `InvalidRequest(String)` | HTTP 400 |
| `Timeout` | Request timeout |
| `NetworkError(String)` | Transport failure (DNS, TCP) |
| `ApiKeyParsingFailed` | Bad API key format |
| `CompletionRequestError(String)` | Provider-level completion error |
| `ContextTooLong` | Model context window exceeded |
| `MaxIterationsExceeded` | Agentic loop iteration cap hit |
| `Other(String)` | Uncategorised |

`HttpError::is_retryable()` returns `true` for `RateLimited`, `ServiceUnavailable`, `Timeout`, and `NetworkError`.

`HttpResult<T>` is a type alias for `Result<T, HttpError>`.

### `logger`

Two initialisation helpers for `tracing-subscriber`:

```rust
// Simple fmt subscriber — switches to JSON when LOG_FORMAT env var is set
rustic_core::set_logger(filter_string);

// fmt + OpenTelemetry — exports to OTLP or Google Cloud Trace
rustic_core::logger::set_logger_with_telemetry(filter, service_name, project_id, endpoint).await?;
```

When `LOG_FORMAT` is set, both functions emit newline-delimited JSON with flattened event fields (compatible with Google Cloud Logging).

### `config::load_content`

Reads a file path or a URL and returns its contents as a `String`. Used by `rustic-boot` to load agent system prompt files referenced in `agents.json`.

## Re-exports

```rust
use rustic_core::{Tool, HttpClient, HttpError, HttpResult, set_logger, load_content};
```
