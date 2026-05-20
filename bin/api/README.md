# api

Axum HTTP server — the entry point for the rustic-ai backend.

## Responsibilities

- Bootstraps the application via `agentic-boot` (config loading, MongoDB connection, Firebase auth, CORS)
- Registers financial analysis tools (`fin-analyse`) backed by a MongoDB storage service
- Mounts route groups: agents, conversations, providers, templates
- Serves on `0.0.0.0:$PORT` (default `8080`)

## Routes

| Group | Prefix | Source |
|---|---|---|
| Agents | `/agents` | `agentic_boot::routes::agents` |
| Conversations | `/conversations` | `agentic_boot::routes::conversation` |
| Providers | `/providers` | `agentic_boot::routes::providers` |
| Templates | `/templates` | `agentic_boot::routes::templates` |

## Configuration

The server reads JSON files from the directory set in `RUSTIC_AI_CONFIG_PATH`:

| File | Purpose |
|---|---|
| `providers.json` | LLM provider definitions |
| `agents.json` | Agent configurations |
| `chat_templates.json` | Reusable conversation templates |
| `mcp_servers_config.json` | MCP server connection details |

## Environment Variables

| Variable | Required | Description |
|---|---|---|
| `RUSTIC_AI_CONFIG_PATH` | Yes | Path to the config directory |
| `RUSTIC_AI_PROJECT_ID` | Yes | Firebase project ID for JWT auth |
| `RUSTIC_AI_DB_NAME` | Yes | MongoDB database for AI state |
| `MONGO_URI` | Yes | MongoDB connection string |
| `OPENAI_API_KEY` | Yes | Used for embeddings |
| `ANTHROPIC_API_KEY` | For Claude | Anthropic API key |
| `GEMINI_API_KEY` | For Gemini | Google AI API key |
| `FINTRACKER_DB_NAME` | Yes | MongoDB database for financial data |
| `PORT` | No | HTTP port (default: `8080`) |
| `RUST_LOG` | No | Log filter (default: `rustic_ai_api=debug,...`) |

## Running

```bash
cargo run --bin api
```

See the [workspace README](../../README.md) for a full example with environment variables.

## CORS

Allowed origins are hardcoded in `main.rs`:

- `http://localhost:4200`
- `http://localhost:4201`
- `http://localhost:4202`
- `https://rustic-ai-rkapps.web.app`
