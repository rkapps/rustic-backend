# rustic-ai-api

Axum HTTP server — the production entry point for the rustic-ai backend.

## What it does

1. Initialises structured logging and OpenTelemetry tracing
2. Connects to MongoDB (separate databases for platform, finance, and economic data)
3. Builds read-only `FinanceService` and `EconomicService` and collects their agent tools
4. Bootstraps the full agent stack via `AgenticBootBuilder` (providers, agents, MCP servers, Firebase auth, CORS)
5. Mounts route groups and listens on `0.0.0.0:$PORT`

## Routes

| Group | Auth required | Description |
|-------|---------------|-------------|
| `GET /agents` | No | List configured agents |
| `GET /agents/:id` | No | Get a single agent |
| `GET /providers` | No | List configured LLM providers |
| `GET /templates` | No | List chat templates |
| `POST /conversations` | Firebase JWT | Create a conversation |
| `GET /conversations` | Firebase JWT | List user conversations |
| `GET /conversations/:id` | Firebase JWT | Get a conversation |
| `DELETE /conversations/:id` | Firebase JWT | Delete a conversation |
| `POST /conversations/:id/turns` | Firebase JWT | Send a turn (blocking) |
| `POST /conversations/:id/turns/stream` | Firebase JWT | Send a turn (SSE streaming) |
| `GET /conversations/:id/turns` | Firebase JWT | Get conversation turns |

## Environment variables

| Variable | Purpose |
|----------|---------|
| `RUSTIC_AI_PROJECT_ID` | Firebase project ID (for JWT validation) |
| `RUSTIC_AI_CONFIG_PATH` | Directory containing config JSON files |
| `OTEL_ENDPOINT` | OpenTelemetry collector endpoint (e.g. `http://localhost:4318`) |
| `MONGO_URI` | MongoDB connection string |
| `RUSTIC_PLATFORM_DB_NAME` | MongoDB database for conversations / agent state |
| `RUSTIC_FINANCE_DB_NAME` | MongoDB database for finance data |
| `RUSTIC_ECONOMIC_DB_NAME` | MongoDB database for economic data |
| `OPENAI_API_KEY` | Used for text embeddings (`text-embedding-3-small`) |
| `PORT` | HTTP port (default `8080`) |
| `RUST_LOG` | Log filter (default `rustic_ai_api=debug,rustic_boot=info,…`) |
| `LOG_FORMAT` | Set to any value to enable JSON cloud logging |

LLM provider API keys (`ANTHROPIC_API_KEY`, `GEMINI_API_KEY`, etc.) are read from
env vars specified per-provider in `providers.json`.

## Config files (in `RUSTIC_AI_CONFIG_PATH`)

| File | Purpose |
|------|---------|
| `providers.json` | LLM provider definitions (model IDs, API key env var names) |
| `agents.json` | Agent configs (system prompt, model, tool bindings, pipeline layout) |
| `chat_templates.json` | Reusable conversation starters |
| `mcp_servers_config.json` | MCP server connection settings |

## Running

```bash
RUSTIC_AI_PROJECT_ID=my-project         \
RUSTIC_AI_CONFIG_PATH=./config          \
OTEL_ENDPOINT=http://localhost:4318     \
MONGO_URI=mongodb://localhost:27017     \
RUSTIC_PLATFORM_DB_NAME=rustic_platform \
RUSTIC_FINANCE_DB_NAME=rustic_finance   \
RUSTIC_ECONOMIC_DB_NAME=rustic_economic \
OPENAI_API_KEY=sk-...                   \
cargo run --bin rustic-ai-api
```
