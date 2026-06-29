# rustic-backend

A Rust workspace for building production-grade agentic AI backends — multi-provider LLM orchestration, tool use, MCP integration, conversation management, finance data, and economic data pipelines.

## Workspace

```text
rustic-backend/
├── crates/
│   ├── rustic-agent       # LLM agent orchestration — the heart of the platform
│   ├── rustic-boot        # Application bootstrap, HTTP server, conversation service
│   ├── rustic-core        # Shared HTTP client, error types, Tool trait, logging
│   ├── rustic-storage     # Backend-agnostic persistence layer (flat-file + MongoDB)
│   ├── rustic-ml          # Embeddings and vector similarity search
│   ├── rustic-providers   # U.S. economic data API clients (FRED, BEA, Census)
│   ├── rustic-economic    # Economic data domain, ingestion pipelines, agent tools
│   └── rustic-finance     # Finance data domain (tickers, charts, news, ML models)
└── bin/
    ├── api                # HTTP API server binary (rustic-ai-api)
    ├── pipeline           # Data ingestion pipeline CLI
    ├── admin              # Database schema management CLI
    └── shared             # Shared startup helpers for the CLI binaries
```

### Crate dependency graph

```text
rustic-core
├── rustic-agent          (LLM orchestration)
├── rustic-storage        (persistence)
├── rustic-ml             (embeddings)
└── rustic-providers      (data API clients)
     ├── rustic-economic  (economic domain + tools)
     └── rustic-finance   (finance domain + tools)

rustic-boot               (wires agent + storage + HTTP together)
```

## Features

- **Multi-provider LLM** — Anthropic (Claude), OpenAI (GPT), Google Gemini, Groq, Local/Ollama
- **Agentic tool loop** — Automatic multi-iteration tool execution (up to 10 rounds) with concurrency control
- **Pipeline agents** — Orchestrator/sub-agent architecture with sequential or parallel execution modes
- **MCP integration** — Connect to Model Context Protocol servers with pluggable adapters
- **Streaming** — Server-Sent Events streaming for all providers and pipeline agents
- **Extended thinking** — Reasoning/thinking token support for Anthropic and Gemini
- **Prompt caching** — Cache control for Anthropic and Gemini with per-turn token cost accounting
- **Persistent conversations** — MongoDB-backed conversation and turn storage with cost tracking
- **Firebase auth** — JWT-based authentication via Firebase
- **Finance & economic data** — Ticker data, price history, news, sentiment, FRED/BEA/Census time series

## Quick start

```bash
cargo build --release

# Run the API server
RUSTIC_AI_PROJECT_ID=<firebase-project>  \
RUSTIC_AI_CONFIG_PATH=./config           \
OTEL_ENDPOINT=http://localhost:4318      \
OPENAI_API_KEY=<key>                     \
MONGO_URI=mongodb://localhost:27017      \
RUSTIC_PLATFORM_DB_NAME=rustic_platform \
RUSTIC_FINANCE_DB_NAME=rustic_finance   \
RUSTIC_ECONOMIC_DB_NAME=rustic_economic \
cargo run --bin rustic-ai-api
```

## API server configuration

The server reads JSON files from `RUSTIC_AI_CONFIG_PATH` at startup:

| File | Purpose |
|------|---------|
| `providers.json` | LLM provider definitions (model IDs, API key env var names) |
| `agents.json` | Agent configs (system prompt, model, tool bindings, pipeline layout) |
| `chat_templates.json` | Reusable conversation starters |
| `mcp_servers_config.json` | MCP server connection settings |

## Key environment variables

| Variable | Used by | Purpose |
|----------|---------|---------|
| `MONGO_URI` | all binaries | MongoDB connection string |
| `RUSTIC_PLATFORM_DB_NAME` | api, admin | Platform (conversations) database |
| `RUSTIC_FINANCE_DB_NAME` | api, pipeline, admin | Finance data database |
| `RUSTIC_ECONOMIC_DB_NAME` | api, pipeline, admin | Economic data database |
| `RUSTIC_AI_CONFIG_PATH` | api | Config file directory |
| `RUSTIC_AI_PROJECT_ID` | api | Firebase project ID |
| `OTEL_ENDPOINT` | api | OpenTelemetry collector endpoint |
| `OPENAI_API_KEY` | api, pipeline | OpenAI key (used for text embeddings) |
| `LOG_FORMAT` | all | Set to any value to switch to JSON cloud logging |
| `PORT` | api | HTTP port (default `8080`) |

## Binaries

| Binary | Description |
|--------|-------------|
| [`rustic-ai-api`](bin/api) | Axum HTTP server — agents, conversations, providers endpoints |
| [`rustic-pipeline`](bin/pipeline) | Data ingestion CLI (EOD prices, news, economic series, embeddings) |
| [`rustic-admin`](bin/admin) | Schema migration and ticker seed loader |

## License

MIT
