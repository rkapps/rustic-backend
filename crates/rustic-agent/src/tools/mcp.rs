use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use rustic_core::http::HttpClient;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tracing::debug;

use crate::{
    client::{
        mcp::MCPServerAdapter,
        rpc::{JsonRpcRequest, JsonRpcResponse},
        tools::ToolDefinition,
    },
    tools::{
        request::{MCPToolCallParamsRequest, MCPToolGetParamsRequest, MCPToolListRequest},
        response::{MCPToolCallResponse, MCPToolDefinition, MCPToolListResponse},
    },
};

/// Connection parameters for a single MCP server.
#[derive(Debug, Clone)]
pub struct MCPServerSetting {
    /// Logical name used to namespace tool definitions (e.g. `"weather"`).
    pub name: String,
    /// HTTP endpoint URL of the MCP server.
    pub url: String,
    /// Bearer token for authentication; leave empty if the server has no auth.
    pub api_key: String,
}

/// Aggregates multiple [`MCPClient`] instances and exposes a unified tool surface to [`Agent`](crate::agents::Agent).
///
/// Tool names are namespaced as `{server_name}___{tool_name}` to avoid collisions
/// across different MCP servers. Both [`register_server`](Self::register_server) and
/// [`register_tool`](Self::register_tool) populate the `definitions` map which is
/// then forwarded to the LLM in every completion request.
#[derive(Debug, Clone)]
pub struct MCPRegistry {
    /// Live client instances keyed by server name.
    pub registry: HashMap<String, MCPClient>,
    /// Namespaced tool definitions ready to be sent to the LLM.
    pub definitions: HashMap<String, ToolDefinition>,
}

impl Default for MCPRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MCPRegistry {
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
            definitions: HashMap::new(),
        }
    }

    /// Register an MCP server using the built-in [`StandardAdapter`].
    ///
    /// Initialises the session handshake and fetches the server's tool list.
    /// Returns the un-namespaced tool definitions; call [`add_definitions`](Self::add_definitions)
    /// or [`register_tool`](Self::register_tool) to make them available to the LLM.
    pub async fn register_server(
        &mut self,
        setting: MCPServerSetting,
    ) -> Result<Vec<ToolDefinition>> {
        let adapter = Box::new(StandardAdapter {});
        self.register_server_with_adapter(setting, adapter).await
    }

    /// Register an MCP server using a custom [`MCPServerAdapter`].
    ///
    /// Use this when the server speaks a non-standard MCP variant. Initialises
    /// the session and returns the tool list (without full parameter schemas).
    pub async fn register_server_with_adapter(
        &mut self,
        setting: MCPServerSetting,
        adapter: Box<dyn MCPServerAdapter>,
    ) -> Result<Vec<ToolDefinition>> {
        // add code to check if server already inserted
        let client =
            MCPClient::new(setting.clone(), adapter).context("Error connecting the MCPClient")?;

        client.initialize().await?;

        let mcp_definitions = client.tool_list().await?;
        // debug!("Definitions: {}", serde_json::to_string(&mcp_definitions)?);
        let definitions: Vec<ToolDefinition> = mcp_definitions
            .into_iter()
            .map(ToolDefinition::from)
            .collect();

        self.registry.insert(setting.name, client);
        Ok(definitions)
    }

    /// Fetch a tool's full parameter schema from the server and cache it in `definitions`.
    ///
    /// The stored key is `{server_name}___{tool_name}`. Returns an error if `server_name`
    /// has not been registered via [`register_server`](Self::register_server).
    pub async fn register_tool(
        &mut self,
        server_name: &str,
        tool_name: &str,
    ) -> Result<ToolDefinition> {
        if let Some(client) = self.registry.get(server_name) {
            let mcp_get_definition = client.tool_get(tool_name).await?;
            let name = format!("{}___{}", server_name, tool_name);

            let tool_definition = ToolDefinition::default_for_mcp(
                "function",
                &name,
                &mcp_get_definition.description.unwrap(),
                mcp_get_definition.input_schema,
            );
            self.definitions.insert(name, tool_definition.clone());

            Ok(tool_definition)
        } else {
            Err(anyhow::anyhow!(
                "Server '{}' has not been registered.",
                server_name
            ))
        }
    }

    /// Insert a batch of tool definitions into the registry, namespacing each name as
    /// `{server_name}___{tool_name}`.
    pub fn add_definitions(&mut self, server_name: &str, definitions: Vec<ToolDefinition>) {
        for mut def in definitions {
            // let name = def.name.clone().replace("-", "_").replace("--", "_");
            let name = def.name.clone();
            let name = format!("{}___{}", server_name, name);
            def.name = name;
            self.definitions.insert(def.name.clone(), def);
        }
    }

    pub fn has_tool(&self, tool_name: &str) -> bool {
        self.definitions.contains_key(tool_name)
    }

    pub fn get_tool(&self, name: &str) -> Option<ToolDefinition> {
        // let guard = self.registry.read().await;
        self.definitions.get(name).cloned()
    }

    /// Invoke a namespaced tool (`{server}___{name}`) and return its JSON result.
    ///
    /// Splits the name on `"___"` to locate the right server client, then delegates
    /// to [`MCPClient::tool_call`]. Returns an error if the server is not registered.
    pub async fn call_tool(&self, tool_name: &str, params: Value) -> Result<Value> {
        let server: Vec<&str> = tool_name.split("___").collect();
        debug!("Server: {:#?}", server);
        let server_name = server[0];
        let tool_call_name = server[1];
        if let Some(client) = self.registry.get(server_name) {
            client.tool_call(tool_call_name, params).await
        } else {
            Err(anyhow::anyhow!(
                "Server '{}' has not been registered.",
                server_name
            ))
        }
    }
}

/// HTTP client for a single MCP server, managing the session lifecycle and all JSON-RPC calls.
///
/// Protocol flow: [`initialize`](Self::initialize) → `notifications/initialized` → tool calls.
/// The session ID returned by the server is stored and sent as `Mcp-Session-Id` on
/// every subsequent request.
#[derive(Debug, Clone)]
pub struct MCPClient {
    pub name: String,
    pub url: String,
    pub api_key: String,
    http_client: HttpClient,
    server_adapter: Arc<Box<dyn MCPServerAdapter>>,
    /// Session token for stateful MCP transports; `None` for stateless servers.
    session_id: Arc<RwLock<Option<String>>>,
}

impl MCPClient {
    /// Construct a new client from a [`MCPServersetting`] and a protocol adapter.
    pub fn new(setting: MCPServerSetting, adapter: Box<dyn MCPServerAdapter>) -> Result<Self> {
        Ok(Self {
            name: setting.name,
            url: setting.url,
            api_key: setting.api_key,
            http_client: HttpClient::new()?,
            server_adapter: Arc::new(adapter),
            session_id: Arc::new(None.into()),
        })
    }

    /// Perform the MCP session handshake: send `initialize`, extract the session ID,
    /// then send `notifications/initialized`.
    pub async fn initialize(&self) -> Result<()> {
        // send initialize request
        let request = self.server_adapter.build_initialize_request();
        let body = serde_json::json!(request);
        let headers = self.get_header().await?;

        let response = self
            .http_client
            .post_request_with_headers::<serde_json::Value>(self.url.clone(), Some(headers), body)
            .await?;

        // extract and store session ID
        if let Some(sid) = self.server_adapter.extract_session_id(&response.headers) {
            let mut session = self.session_id.write().await;
            *session = Some(sid);
        }

        // send initialized notification
        let notif = self.server_adapter.build_initialized_notification();
        let body = serde_json::json!(notif);
        let headers = self.get_header().await?;

        self.http_client
            .post_notification(self.url.clone(), Some(headers), body)
            .await?;

        Ok(())
    }

    async fn get_header(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers.insert(
            "Accept",
            HeaderValue::from_static("application/json, text/event-stream"),
        );

        if !self.api_key.is_empty() {
            headers.insert(
                "Authorization",
                HeaderValue::from_str(&format!("Bearer {}", self.api_key))?,
            );
        }

        // include session ID if we have one
        let session = self.session_id.read().await;
        if let Some(sid) = session.as_ref() {
            headers.insert("Mcp-Session-Id", HeaderValue::from_str(sid)?);
        }

        Ok(headers)
    }

    async fn tool_list(&self) -> Result<Vec<MCPToolDefinition>> {
        let request = self.server_adapter.build_tool_list_request();
        let body = serde_json::json!(request);
        let headers = self.get_header().await?;
        let response = self
            .http_client
            .post_request::<JsonRpcResponse<MCPToolListResponse>>(
                self.url.clone(),
                Some(headers),
                body,
            )
            .await?;

        Ok(response.result.tools)
    }

    async fn tool_get(&self, name: &str) -> Result<MCPToolDefinition> {
        let request = self.server_adapter.build_tool_get_request(name);
        let body = serde_json::json!(request);
        let headers = self.get_header().await?;

        let response = self
            .http_client
            .post_request::<JsonRpcResponse<MCPToolDefinition>>(
                self.url.clone(),
                Some(headers),
                body,
            )
            .await?;

        Ok(response.result)
    }

    async fn tool_call(&self, name: &str, params: Value) -> Result<Value> {
        let request = self.server_adapter.build_tool_call_request(name, params);
        let body = serde_json::json!(request);
        let headers = self.get_header().await?;
        debug!("Tool_call request: {:#?}", request);
        let response = self
            .http_client
            .post_request::<JsonRpcResponse<MCPToolCallResponse>>(
                self.url.clone(),
                Some(headers),
                body,
            )
            .await?;

        debug!("Raw text from MCP: {:?}", response.result.content[0].text);

        let value = self
            .server_adapter
            .parse_tool_call_response(response.result.content[0].clone().text);

        Ok(value)
    }
}

/// The default [`MCPServerAdapter`] that follows the standard MCP JSON-RPC protocol.
///
/// Constructs requests using `tools/list`, `tools/get`, and `tools/call` methods,
/// and extracts the session ID from the `Mcp-Session-Id` response header.
#[derive(Debug)]
struct StandardAdapter {}

impl MCPServerAdapter for StandardAdapter {
    fn build_tool_list_request(&self) -> JsonRpcRequest {
        JsonRpcRequest::default(
            "tools/list".to_string(),
            serde_json::to_value(MCPToolListRequest {}).ok(),
        )
    }

    fn parse_tool_list_response(&self, text: String) -> Result<String> {
        Ok(text)
    }

    fn build_tool_get_request(&self, name: &str) -> JsonRpcRequest {
        let tool_get = MCPToolGetParamsRequest {
            tool_name: name.to_string(),
        };
        JsonRpcRequest::default("tools/get".to_string(), serde_json::to_value(tool_get).ok())
    }
    fn parse_tool_get_response(&self, text: String) -> Result<String> {
        Ok(text)
    }

    fn build_tool_call_request(&self, name: &str, params: Value) -> JsonRpcRequest {
        let tool_call = MCPToolCallParamsRequest {
            name: name.to_string(),
            arguments: params,
        };
        JsonRpcRequest::default(
            "tools/call".to_string(),
            serde_json::to_value(tool_call).ok(),
        )
    }

    fn parse_tool_call_response(&self, text: String) -> Value {
        serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text))
    }

    fn build_initialize_request(&self) -> JsonRpcRequest {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "agentic-core",
                "version": "1.0.0"
            }
        });
        JsonRpcRequest::default("initialize".to_string(), Some(params))
    }

    fn build_initialized_notification(&self) -> JsonRpcRequest {
        // notifications have no id
        JsonRpcRequest::new(
            "2.0".to_string(),
            "notifications/initialized".to_string(),
            None,
            None,
        )
    }

    fn extract_session_id(&self, headers: &HeaderMap) -> Option<String> {
        headers
            .get("Mcp-Session-Id")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
    }
}
