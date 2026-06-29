# rustic-agent

LLM agent orchestration for the rustic-ai platform. Provides a unified agent abstraction over Anthropic (Claude), OpenAI (GPT), Google Gemini, Groq, and local Anthropic-compatible servers (Ollama) with full tool use, MCP integration, and both single-agent and multi-agent pipeline topologies.

## Features

- **Multi-provider** — Anthropic, OpenAI, Gemini, Groq, Local/Ollama behind a single `LlmClient` trait
- **Agentic tool loop** — Automatic multi-iteration tool dispatch (up to 10 rounds) with concurrency control and per-call timeouts; tools run concurrently with a semaphore limit of 3
- **Pipeline agents** — Orchestrator/sub-agent topology where an orchestrator LLM decides which sub-agents to run each stage, in sequential or parallel mode, up to 10 pipeline iterations
- **MCP support** — Connect to Model Context Protocol servers with a pluggable adapter interface; tools are namespaced per server to avoid collisions
- **Streaming** — SSE streaming for all providers via `complete_with_streaming`; pipeline agents forward orchestrator status chunks alongside content
- **Extended thinking** — `ReasoningEffort` (None / Low / Medium / High) for Anthropic and Gemini
- **Prompt caching** — Cache-control for Anthropic and Gemini with per-turn token cost breakdown
- **Client caching** — `AgentService` caches provider clients by `"{llm}:{model}"` key; multiple agents sharing the same model reuse the same connection

## Module layout

```text
rustic-agent/src/
├── agents/
│   ├── agent.rs     — Agent struct; complete() and complete_with_streaming() tool loops
│   ├── domain.rs    — AgentInput, CompletionTurn, StageDecision, ExecutionMode
│   ├── helper.rs    — Message building, JSON helpers, status formatting
│   └── runner.rs    — Runnable trait, SingleAgent, PipeLineAgent
├── client/
│   ├── llm.rs       — LlmClient trait, LlmProvider
│   ├── message.rs   — Message enum (User, Assistant, Thought, ToolCall, ToolOutput)
│   ├── mcp.rs       — MCPServerAdapter trait
│   ├── preset.rs    — Preset enum (Fast, Balanced, Precise, Thorough, Local)
│   ├── provider.rs  — Provider enum (constructor helpers)
│   ├── request.rs   — CompletionRequest, ReasoningEffort
│   ├── response.rs  — CompletionResponse, CompletionChunkResponse, token usage
│   ├── rpc.rs       — JSON-RPC 2.0 envelope types
│   └── tools.rs     — ToolDefinition, ToolCallRequest
├── providers/
│   ├── anthropic/   — Claude via Messages API
│   ├── gemini/      — Gemini via Generate Content API
│   ├── groq/        — Groq via OpenAI-compatible API
│   ├── local/       — Anthropic-compatible local servers (Ollama)
│   └── openai/      — GPT via Responses API
├── services/
│   ├── agent.rs     — AgentService: builds Runnable instances from registries
│   ├── builder.rs   — AgentBuilder: fluent builder with client caching and presets
│   ├── config/      — JSON config types (AgentConfig, ProviderConfig, MCPServerConfig)
│   └── registry/    — In-memory registries (ProviderRegistry, AgentRegistry)
└── tools/
    ├── tool.rs      — ToolRegistry (in-process tools)
    └── mcp.rs       — MCPRegistry, MCPClient, StandardAdapter
```

## Usage

### Quick start — direct agent construction

```rust
use std::sync::Arc;
use rustic_agent::{
    Agent, AgentBuilder, AgentService, Message,
    Provider, Preset,
};

// Minimal: build via AgentService (handles client caching)
let service = AgentService::default();
let agent = service
    .builder()
    .with_provider(Provider::anthropic(api_key, "claude-sonnet-4-6"))
    .await?
    .with_system_prompt("You are a helpful assistant.".to_string())
    .with_preset(Preset::Balanced)
    .build()
    .await?;

let messages = vec![Message::user("What is the capital of France?".to_string())];
let response = agent.complete(&messages, None).await?;
println!("{}", response.text().unwrap_or_default());
```

### Completion methods

`Agent` exposes two completion methods; both drive the full agentic tool-use loop:

| Method | Streaming |
|--------|-----------|
| `complete(&messages, last_response_id)` | No — returns `CompletionResponse` |
| `complete_with_streaming(&messages, last_response_id)` | Yes — returns `ReceiverStream<HttpResult<CompletionChunkResponse>>` |

Both methods:

1. Collect tool definitions from `ToolRegistry` and `MCPRegistry`
2. Call the LLM with the full message history
3. Execute any requested tool calls concurrently (semaphore-bounded)
4. Append tool results to the conversation and loop, up to 10 iterations
5. Return the final response (or stream a stop chunk with aggregated token usage)

### Providers

```rust
use rustic_agent::Provider;

// Use the Provider enum for a clean API
Provider::anthropic(api_key, "claude-sonnet-4-6")
Provider::openai(api_key, "gpt-4o")
Provider::gemini(api_key, "gemini-2.0-flash")
Provider::groq(api_key, "llama-3.1-70b-versatile")
Provider::local("llama3.2", "http://localhost:11434")
Provider::ollama("llama3.2")   // shorthand for local at default Ollama port
```

### Presets

`Preset` bundles temperature, max tokens, cache, and reasoning effort:

| Preset | Temperature | Max tokens | Cache | Reasoning |
|--------|-------------|------------|-------|-----------|
| `Fast` | 0.7 | 1 024 | No | None |
| `Balanced` | 0.5 | 8 192 | Yes | Low |
| `Precise` | 0.2 | 65 536 | Yes | Low |
| `Thorough` | 0.1 | 65 536 | Yes | High |
| `Local` | 0.7 | 4 096 | No | None |

### Messages

```rust
use rustic_agent::Message;

Message::user("Hello".to_string())
Message::assistant("Hi there!".to_string())

// Managed automatically by the tool loop — you do not create these directly
Message::ToolCall  { call_id, name, arguments }
Message::ToolOutput { call_id, name, output }
Message::Thought   { content }   // reasoning tokens for Gemini multi-turn
```

`last_response_id` passed to `complete` / `complete_with_streaming` threads
provider-side conversation state (OpenAI `previous_response_id`, Gemini `previous_interaction_id`).

### Custom tools

Implement the `Tool` trait from `rustic-core`:

```rust
use async_trait::async_trait;
use rustic_core::Tool;

#[derive(Debug)]
struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> String { "get_weather".to_string() }
    fn description(&self) -> String { "Get current weather for a city.".to_string() }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": { "city": { "type": "string", "description": "City name" } },
            "required": ["city"]
        })
    }
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::json!({ "temperature": "22°C", "city": args["city"] }))
    }
}
```

Register on the builder:

```rust
service.builder()
    .with_tool(WeatherTool)       // statically typed
    .with_tool_boxed(some_arc)   // Arc<dyn Tool>
    .with_tools(vec![...])        // Vec<Arc<dyn Tool>>
    // ...
    .build().await?;
```

### MCP servers

```rust
use rustic_agent::tools::mcp::{MCPRegistry, MCPServerSetting};

let setting = MCPServerSetting {
    name: "search".to_string(),
    url: "http://localhost:8081/mcp".to_string(),
    api_key: String::new(),
};

// Option A: register all tools the server exposes
let mut mcp = MCPRegistry::new();
let definitions = mcp.register_server(setting.clone()).await?;
mcp.add_definitions(&setting.name, definitions); // namespaced as "search___<tool_name>"

// Option B: register individual tools selectively
mcp.register_tool("search", "web_search").await?;

// Attach to builder
service.builder()
    .with_mcp_registry(setting, MyAdapter {}).await?
    // or to filter the shared registry to specific tools:
    .with_filtered_mcp(mcp)
    // ...
    .build().await?;
```

### Pipeline agents

`PipeLineAgent` implements an orchestrator/sub-agent loop. The orchestrator LLM returns a `StageDecision` JSON each iteration; `PipeLineAgent` runs the nominated sub-agents and loops until `stop: true`.

```json
{
  "agents": [
    { "id": "researcher", "goal": "Find recent news about AAPL" },
    { "id": "analyst",    "goal": "Summarise the news for an investor" }
  ],
  "execution": "sequential",
  "stop": false,
  "reasoning": "Need research before analysis"
}
```

- **Sequential** — sub-agents run in order; each receives the previous output as context
- **Parallel** — sub-agents run concurrently (semaphore limit 5, 120 s timeout each)
- Final stage (`"stop": true`) must list exactly one agent and use `"sequential"` execution

`AgentService.build_runnable` reads an `AgentConfig` and constructs either a `SingleAgent` or `PipeLineAgent` wrapped in `Arc<dyn Runnable>`.

### Streaming

```rust
use tokio_stream::StreamExt;

let mut stream = agent.complete_with_streaming(&messages, None).await?;
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    if chunk.is_final {
        println!("\nTokens: {:?}", chunk.usage);
        break;
    }
    print!("{}", chunk.content);
}
```

Pipeline agents send `CompletionChunkResponse::status(...)` chunks (e.g. `"  ✅ 1.2s\n"`) between content chunks so the caller can show orchestration progress.

## Examples

```bash
cd crates/rustic-agent

# Multi-turn chat (Gemini, no tools)
GEMINI_API_KEY=<key> cargo run --example completion

# Custom tool (Gemini + WeatherTool)
GEMINI_API_KEY=<key> cargo run --example completion_with_tools

# Remote MCP server (OpenAI + Apify MCP via AgentService)
OPENAI_API_KEY=<key> APIFY_API_KEY=<key> cargo run --example completion_with_mcp
```

## Supported providers and models

### Anthropic

| Constant | Model ID |
|----------|----------|
| `anthropic::MODEL_CLAUDE_SONNET_4_6` | `claude-sonnet-4-6` |
| `anthropic::MODEL_CLAUDE_SONNET_4_5` | `claude-sonnet-4-5` |
| `anthropic::MODEL_CLAUDE_HAIKU_4_5` | `claude-haiku-4-5` |
| `anthropic::MODEL_CLAUDE_OPUS_4_5` | `claude-opus-4-5` |

### OpenAI

| Constant | Model ID |
|----------|----------|
| `openai::MODEL_GPT_4O` | `gpt-4o` |
| `openai::MODEL_GPT_4O_MINI` | `gpt-4o-mini` |

### Google Gemini

| Constant | Model ID |
|----------|----------|
| `gemini::MODEL_GEMINI_2_FLASH` | `gemini-2.0-flash` |
| `gemini::MODEL_GEMINI_2_PRO` | `gemini-2.0-pro` |
| `gemini::MODEL_GEMINI_2_5_FLASH` | `gemini-2.5-flash-preview-05-20` |

### Groq

Groq is configured via `Provider::groq(api_key, model_id)` where `model_id` is any Groq-hosted model identifier (e.g. `"llama-3.1-70b-versatile"`).
