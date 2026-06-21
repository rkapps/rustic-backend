# rustic-backend

Backend service for multi-provider LLM agents with persistent storage, MCP tool integration, and a REST API.

## Overview

A Rust workspace that provides a production-ready agentic AI backend. It exposes a REST API for managing conversations and agents across multiple LLM providers, backed by MongoDB and secured with Firebase authentication.

## Workspace

| Crate                 | Description                                                          |
| --------------------- | -------------------------------------------------------------------- |
| `bin/api`             | Axum HTTP server — entry point for all REST endpoints                |
| `crates/rustic-agent` | Multi-provider LLM client library (Anthropic, OpenAI, Gemini, Local) |
| `crates/rustic-core`  | Shared HTTP client, error types, and utilities                       |
| `crates/rustic-boot`  | Application bootstrap: config loading, routing, startup              |

## Features

- **Multi-provider LLM support** — Anthropic (Claude), OpenAI (GPT), Google Gemini, Local/Ollama
- **Agentic tool loop** — Automatic multi-iteration tool execution, up to 10 rounds, with concurrency control
- **MCP integration** — Connect to Model Context Protocol servers with pluggable adapters
- **Streaming responses** — SSE streaming for all providers
- **Extended thinking** — Reasoning/thinking token support for Anthropic and Gemini
- **Prompt caching** — Built-in cache control for Anthropic with detailed token accounting
- **Persistent storage** — MongoDB-backed conversation and agent state
- **Firebase auth** — JWT-based authentication via Firebase
- **Async** — Built on Tokio

## Quick Start

### Prerequisites

- Rust 1.85+
- MongoDB instance
- Firebase project
- API keys for the LLM providers you want to use

### Build

```bash
git clone https://github.com/rkapps/rustic-ai-backend-rs.git
cd rustic-ai-backend-rs
cargo build --release
```

### Environment Variables

| Variable                | Required                | Description                                     |
| ----------------------- | ----------------------- | ----------------------------------------------- |
| `ANTHROPIC_API_KEY`     | For Claude              | Anthropic API key                               |
| `OPENAI_API_KEY`        | For OpenAI + embeddings | OpenAI API key                                  |
| `GEMINI_API_KEY`        | For Gemini              | Google AI API key                               |
| `MONGO_URI`             | Yes                     | MongoDB connection string                       |
| `RUSTIC_AI_DB_NAME`     | Yes                     | MongoDB database name for the AI backend        |
| `FINTRACKER_DB_NAME`    | Yes                     | MongoDB database name for financial data        |
| `RUSTIC_AI_CONFIG_PATH` | Yes                     | Path to the config directory                    |
| `RUSTIC_AI_PROJECT_ID`  | Yes                     | Firebase project ID                             |
| `PORT`                  | No                      | HTTP port (default: `8080`)                     |
| `RUST_LOG`              | No                      | Log filter (default: `rustic_ai_api=debug,...`) |

### Run

```bash
RUSTIC_AI_CONFIG_PATH=./config \
RUSTIC_AI_PROJECT_ID=my-firebase-project \
MONGO_URI=mongodb://localhost:27017 \
RUSTIC_AI_DB_NAME=rustic_ai \
FINTRACKER_DB_NAME=fin_tracker \
OPENAI_API_KEY=sk-... \
cargo run --bin api
```

## Configuration

The server reads JSON config files from `RUSTIC_AI_CONFIG_PATH` at startup:

| File                      | Purpose                                                    |
| ------------------------- | ---------------------------------------------------------- |
| `providers.json`          | LLM provider definitions (model, API key references)       |
| `agents.json`             | Agent configurations (system prompt, model, tool bindings) |
| `chat_templates.json`     | Reusable conversation templates                            |
| `mcp_servers_config.json` | MCP server connection details                              |

## rustic-agent

The `rustic-agent` crate is a standalone multi-provider LLM client library. It can be used independently of the API server. It includes both a low-level `Agent` struct and a higher-level `AgentService` / `AgentBuilder` layer that handles client caching, preset configuration, and shared tool registries.

### Providers

```rust
use rustic_agent::providers::anthropic::{completion::AnthropicClient, MODEL_CLAUDE_SONNET_4_6};
use rustic_agent::providers::openai::{completion::OpenAIClient, MODEL_GPT_5_4_MINI};
use rustic_agent::providers::gemini::{completion::GeminiClient, MODEL_GEMINI_3_FLASH_PREVIEW};
use rustic_agent::providers::local::completion::LocalClient;

let anthropic = AnthropicClient::new(api_key)?;
let openai    = OpenAIClient::new(api_key)?;
let gemini    = GeminiClient::new(api_key)?;
let local     = LocalClient::anthropic_compat("http://localhost:11434".to_string())?;
```

### Agent

```rust
use std::sync::Arc;
use rustic_agent::{
    agents::Agent,
    client::{message::Message, request::ReasoningEffort},
    providers::anthropic::{completion::AnthropicClient, MODEL_CLAUDE_SONNET_4_6},
    tools::{mcp::MCPRegistry, tool::ToolRegistry},
};

let agent = Agent {
    client: Arc::new(AnthropicClient::new(api_key)?),
    llm: "anthropic".to_string(),
    model: MODEL_CLAUDE_SONNET_4_6.to_string(),
    system_prompt: Some("You are a helpful assistant.".to_string()),
    temperature: 0.7,
    max_tokens: 4096,
    enable_cache: true,
    reasoning_effort: ReasoningEffort::Medium,
    tool_registry: Arc::new(ToolRegistry::new()),
    mcp_registry: Arc::new(MCPRegistry::new()),
};

let messages = vec![Message::User {
    content: "Hello!".to_string(),
    response_id: None,
}];

let response = agent.complete(&messages).await?;
```

### Completion Methods

| Method                                     | Tools | Streaming |
| ------------------------------------------ | ----- | --------- |
| `complete(&messages)`                      | No    | No        |
| `complete_with_stream(&messages)`          | No    | Yes       |
| `complete_with_tools(&messages)`           | Yes   | No        |
| `complete_with_tools_streaming(&messages)` | Yes   | Yes       |

### Custom Tools

```rust
use async_trait::async_trait;
use rustic_agent::client::tools::Tool;

#[derive(Debug)]
struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> String { "get_weather".to_string() }
    fn description(&self) -> String { "Get current weather for a city".to_string() }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": { "city": { "type": "string" } },
            "required": ["city"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::json!({ "temperature": "22°C", "city": args["city"] }))
    }
}

let mut registry = ToolRegistry::new();
registry.register_tool(WeatherTool);
```

### MCP Servers

```rust
use rustic_agent::tools::mcp::{MCPRegistry, MCPServerSetting};

let mut mcp = MCPRegistry::new();

let setting = MCPServerSetting {
    name: "docs".to_string(),
    url: "http://localhost:8081/mcp".to_string(),
    api_key: "".to_string(),
};

// Initialises the session and bulk-registers all tools.
// Each tool is namespaced as "docs___<tool_name>" to avoid collisions.
let definitions = mcp.register_server(setting.clone()).await?;
mcp.add_definitions(&setting.name, definitions);

// Or register individual tools selectively
mcp.register_tool("docs", "search").await?;
```

### Streaming

```rust
use tokio_stream::StreamExt;

let mut stream = agent.complete_with_tools_streaming(&messages).await?;
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    if chunk.is_final {
        println!("Usage: {:?}", chunk.usage);
        break;
    }
    print!("{}", chunk.content);
}
```

### Examples

| Example                 | Provider | Description                                  |
| ----------------------- | -------- | -------------------------------------------- |
| `completion`            | Gemini   | Multi-turn chat with `response_id` threading |
| `completion_with_tools` | Gemini   | Custom local tool (`GetWeatherTool`)         |
| `completion_with_mcp`   | OpenAI   | Remote MCP server (Apify) via `AgentService` |

```bash
cd crates/rustic-agent
GEMINI_API_KEY=<key> cargo run --example completion
GEMINI_API_KEY=<key> cargo run --example completion_with_tools
OPENAI_API_KEY=<key> APIFY_API_KEY=<key> cargo run --example completion_with_mcp
```

## Supported Models

### Anthropic

| Constant                  | Model               |
| ------------------------- | ------------------- |
| `MODEL_CLAUDE_SONNET_4_6` | `claude-sonnet-4-6` |
| `MODEL_CLAUDE_SONNET_4_5` | `claude-sonnet-4-5` |
| `MODEL_CLAUDE_HAIKU_4_5`  | `claude-haiku-4-5`  |
| `MODEL_CLAUDE_OPUS_4_6`   | `claude-opus-4-6`   |

### OpenAI

| Constant                       | Model                    |
| ------------------------------ | ------------------------ |
| `MODEL_GPT_5_4`                | `gpt-5.4`                |
| `MODEL_GPT_5_4_MINI`           | `gpt-5.4-mini`           |
| `MODEL_GPT_5_4_NANO`           | `gpt-5.4-nano`           |
| `MODEL_TEXT_EMBEDDING_3_SMALL` | `text-embedding-3-small` |

### Google Gemini

| Constant                              | Model                           |
| ------------------------------------- | ------------------------------- |
| `MODEL_GEMINI_3_FLASH_PREVIEW`        | `gemini-3-flash-preview`        |
| `MODEL_GEMINI_3_1_FLASH_LITE_PREVIEW` | `gemini-3.1-flash-lite-preview` |
| `MODEL_GEMINI_3_1_PRO_PREVIEW`        | `gemini-3.1-pro-preview`        |
| `MODEL_GEMINI_EMBEDDING_001`          | `gemini-embedding-001`          |

## License

MIT
