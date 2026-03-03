pub mod api;
pub mod client_manager;
pub mod db;
pub mod health_monitor;
pub mod server;
pub mod tool_manager;
pub mod tool_syncer;
pub mod transport;
pub mod types;

#[allow(unused_imports)]
pub use db::McpDb;
pub use types::*;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use uuid::Uuid;

use client_manager::ClientManager;

/// MCP Gateway manager — orchestrates client connections, tool discovery, and execution.
///
/// Wraps `ClientManager` for connection lifecycle and provides the public API
/// consumed by route handlers and pipeline integration.
#[allow(dead_code)]
pub struct McpManager {
    clients: Arc<RwLock<HashMap<Uuid, McpClientState>>>,
    client_manager: ClientManager,
    db: Option<Arc<McpDb>>,
}

#[allow(dead_code)]
impl McpManager {
    /// Create a new McpManager with a database connection.
    pub fn new(db: Arc<McpDb>, default_timeout_secs: u64) -> Self {
        let clients = Arc::new(RwLock::new(HashMap::new()));
        let client_manager = ClientManager::new(Arc::clone(&clients), default_timeout_secs);
        Self {
            clients,
            client_manager,
            db: Some(db),
        }
    }

    /// Create a new McpManager without database (for testing).
    pub fn new_without_db() -> Self {
        let clients = Arc::new(RwLock::new(HashMap::new()));
        let client_manager = ClientManager::new(Arc::clone(&clients), 30);
        Self {
            clients,
            client_manager,
            db: None,
        }
    }

    /// Initialize: load all clients from DB and connect each enabled one.
    pub async fn initialize(&self) {
        let db = match &self.db {
            Some(db) => db,
            None => {
                tracing::info!("McpManager initialized without database");
                return;
            }
        };

        match db.list_clients().await {
            Ok(configs) => {
                tracing::info!(count = configs.len(), "Loading MCP clients from database");
                for config in &configs {
                    if let Err(e) = self.client_manager.add_client(config).await {
                        tracing::error!(
                            name = %config.name,
                            error = %e,
                            "Failed to initialize MCP client"
                        );
                    }
                }
                let connected = self
                    .clients
                    .read()
                    .await
                    .values()
                    .filter(|c| c.connection_state == McpConnectionState::Connected)
                    .count();
                tracing::info!(
                    total = configs.len(),
                    connected = connected,
                    "MCP Gateway initialized"
                );
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to load MCP clients from database");
            }
        }
    }

    /// Add a new MCP client. Persists to DB if available, then connects.
    pub async fn add_client(&self, config: &McpClientConfig) -> Result<()> {
        // Persist to DB
        if let Some(db) = &self.db {
            db.create_client(config).await?;
        }

        // Connect
        self.client_manager.add_client(config).await
    }

    /// Remove an MCP client. Disconnects and removes from DB.
    pub async fn remove_client(&self, id: Uuid) -> Result<()> {
        self.client_manager.remove_client(id).await?;

        if let Some(db) = &self.db {
            db.delete_client(id).await?;
        }

        Ok(())
    }

    /// Reconnect an existing client (close + re-establish transport).
    pub async fn reconnect_client(&self, id: Uuid) -> Result<()> {
        self.client_manager.reconnect_client(id).await
    }

    /// Update a client's configuration. Persists to DB and reconnects if needed.
    pub async fn update_client(&self, config: &McpClientConfig) -> Result<()> {
        if let Some(db) = &self.db {
            db.update_client(config).await?;
        }

        self.client_manager.update_client(config).await
    }

    /// Get all clients with their current state and tools.
    pub async fn get_clients(&self) -> Vec<McpClientResponse> {
        let clients = self.clients.read().await;
        clients
            .values()
            .map(|state| McpClientResponse {
                config: state.config.clone(),
                connection_state: state.connection_state.clone(),
                tools: state.tools.values().cloned().collect(),
                last_error: state.last_error.clone(),
            })
            .collect()
    }

    /// Get a single client's state by ID.
    pub async fn get_client(&self, id: Uuid) -> Option<McpClientResponse> {
        let clients = self.clients.read().await;
        clients.get(&id).map(|state| McpClientResponse {
            config: state.config.clone(),
            connection_state: state.connection_state.clone(),
            tools: state.tools.values().cloned().collect(),
            last_error: state.last_error.clone(),
        })
    }

    /// Send a JSON-RPC request to a specific client's transport.
    pub async fn send_request(
        &self,
        client_id: Uuid,
        request: &JsonRpcRequest,
    ) -> Result<JsonRpcResponse> {
        let transports = self.client_manager.transports().read().await;
        let transport = transports
            .get(&client_id)
            .ok_or_else(|| anyhow::anyhow!("No transport for client {}", client_id))?;

        transport
            .send_request(request)
            .await
            .map_err(|e| anyhow::anyhow!("Transport error: {}", e))
    }

    /// Get a reference to the clients RwLock (for tool_manager, health_monitor, etc.).
    pub fn clients_ref(&self) -> Arc<RwLock<HashMap<Uuid, McpClientState>>> {
        Arc::clone(&self.clients)
    }

    /// Get a reference to the transports RwLock (for tool_manager, server, etc.).
    pub fn transports_ref(&self) -> Arc<RwLock<HashMap<Uuid, Box<dyn transport::McpTransport>>>> {
        Arc::clone(self.client_manager.transports())
    }

    /// Get available tools formatted for chat completion injection.
    pub async fn get_available_tools(
        &self,
        headers: &axum::http::HeaderMap,
    ) -> Vec<serde_json::Value> {
        tool_manager::get_available_tools(&self.clients, headers).await
    }

    /// Execute a tool by its prefixed name.
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
        headers: &axum::http::HeaderMap,
        timeout_secs: u64,
    ) -> Result<serde_json::Value, String> {
        let transports = self.transports_ref();
        tool_manager::execute_tool(
            &self.clients,
            &transports,
            tool_name,
            arguments,
            headers,
            timeout_secs,
        )
        .await
    }

    /// Get all tools in JSON-RPC format (for /mcp server).
    pub async fn get_all_tools_jsonrpc(&self) -> serde_json::Value {
        tool_manager::get_all_tools_jsonrpc(&self.clients).await
    }

    /// Route a tool call via JSON-RPC (for /mcp server).
    pub async fn call_tool_jsonrpc(
        &self,
        name: &str,
        arguments: serde_json::Value,
        timeout_secs: u64,
    ) -> Result<serde_json::Value, String> {
        let transports = self.transports_ref();
        tool_manager::call_tool_jsonrpc(&self.clients, &transports, name, arguments, timeout_secs)
            .await
    }

    /// Shutdown all clients and background tasks.
    pub async fn shutdown(&self) {
        tracing::info!("McpManager shutting down");
        self.client_manager.shutdown_all().await;
    }
}
