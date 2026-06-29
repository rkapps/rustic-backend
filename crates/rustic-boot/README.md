# rustic-boot

Application bootstrap layer for the rustic-ai platform. Wires agent configuration, MongoDB storage, Firebase auth, MCP servers, and tool registration into a running Axum HTTP server with a single builder call.

## What it does

`rustic-boot` sits between `rustic-agent` (which handles LLM calls) and the API binary (which handles HTTP). It:

- Loads config files from disk (`providers.json`, `agents.json`, `mcp_servers_config.json`, `chat_templates.json`)
- Builds the `ProviderRegistry`, `AgentRegistry`, `MCPRegistry`, and `ToolRegistry`
- Validates that every agent's tool references are resolvable at startup
- Connects to MongoDB and sets up conversation storage
- Fetches and caches Firebase public keys for JWT authentication
- Mounts built-in routes and delegates to the caller for app-specific routes
- Starts the Axum server with CORS and OpenTelemetry trace middleware

## Key types

### `AgenticBootBuilder`

Fluent builder — the primary entry point:

```rust
use rustic_boot::boot::AgenticBootBuilder;

AgenticBootBuilder::new()
    .config_dir("/etc/rustic".to_string())
    .providers("providers.json".to_string())
    .agents_config("agents.json".to_string())
    .mcp_config("mcp_servers_config.json".to_string())
    .chat_templates("chat_templates.json".to_string())
    .firebase_project_id("my-firebase-project")
    .mongo_database(mongo_uri, mongo_db)
    .cors_origins(vec!["https://myapp.example.com"])
    .tools(my_tools)
    .serve(&addr, extend_state, public_routes, protected_routes)
    .await?;
```

`serve` takes four closures:

| Closure | Signature | Purpose |
|---------|-----------|---------|
| `extend_state` | `FnOnce(BootState) -> S` | Wrap `BootState` in your own `AppState` |
| `public` | `FnOnce(Router<S>, S) -> Router<S>` | Attach unauthenticated routes |
| `protected` | `FnOnce(Router<S>, S) -> Router<S>` | Attach Firebase-authenticated routes |

Call `.build().await?` instead of `.serve(...)` to get a `BootState` without starting a server.

### `BootState`

Shared state injected into every Axum handler:

```rust
pub struct BootState {
    pub agent_service: Arc<AgentService>,
    pub conversation_service: Option<Arc<ConversationService>>,
    pub chat_templates: Vec<ChatTemplate>,
    pub firebase_keys: Arc<RwLock<FirebaseKeyCache>>,
    pub firebase_project_id: String,
}
```

`conversation_service` is `None` when `.mongo_database(...)` was not called on the builder.

### `ConversationService`

Manages the full conversation lifecycle over MongoDB:

| Method | Description |
|--------|-------------|
| `create_conversation` | Create a new conversation (Chat or Agent type) |
| `send_turn` | Send a prompt, run the agent, persist the turn, return the response |
| `send_turn_streaming` | Same as above but returns a streaming `CompletionStreamResponse` |
| `get_conversations` | List conversations for a user with optional filters |
| `get_turns` | Fetch all turns in a conversation |
| `save_turn` | Persist a turn with token usage and per-token cost breakdown |
| `recalculate_conversation_usage_cost` | Recompute costs for all turns in a conversation |

### Built-in routes

```rust
use rustic_boot::routes::{
    agents::agent_routes,
    conversation::conversation_routes,
    providers::provider_routes,
    templates::template_routes,
};
```

| Route group | Endpoints |
|-------------|-----------|
| `agent_routes()` | List agents, get agent by ID |
| `conversation_routes(state)` | CRUD conversations, send turns (streaming + non-streaming) |
| `provider_routes()` | List configured LLM providers |
| `template_routes()` | List chat templates |

### Firebase auth

`firebase_auth_middleware` validates the Firebase JWT in the `Authorization` header
and injects `FirebaseClaims` into request extensions. Applied automatically by
`conversation_routes`.

### Schema management

```rust
use rustic_boot::schema::update_rustic_platform;

// Creates MongoDB indexes for conversations and turns.
update_rustic_platform(&mongo_uri, &db_name).await?;
```

## Dependencies

- `rustic-agent` — `AgentService`, provider clients, `MCPRegistry`
- `rustic-core` — `Tool` trait, `HttpError`
- `rustic-storage` — `BootStorageManager` (MongoDB conversation / turn persistence)
