//! This module contains the implementation for the MCP (Model Control Protocol) service.

use std::{ops::Deref, str::FromStr, sync::Arc};

use hyper::{
    HeaderMap,
    header::{HeaderName, HeaderValue},
};
use rmcp::{
    RoleClient, ServiceExt,
    model::Tool,
    service::RunningService,
    transport::{StreamableHttpClientTransport, TokioChildProcess, streamable_http_client::StreamableHttpClientTransportConfig},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::process::Command;

use crate::base::types::Res;

// Types.

/// Struct that represents a server in the MCP configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    pub name: String,
    pub config: McpServerConfig,
}

/// Enum that represents the configuration of an MCP server, which can be either local or remote.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpServerConfig {
    Local {
        command: String,
        args: Vec<String>,
        envs: Option<Vec<(String, String)>>,
    },
    Remote {
        url: String,
        headers: Option<Vec<(String, String)>>,
    },
}

/// Struct that represents and MCP, and its tools.
#[derive(Debug, Clone)]
pub struct Mcp {
    pub name: String,
    pub client: Arc<RunningService<RoleClient, ()>>,
    pub tools: Vec<Tool>,
}

/// Struct for McpClient.
///
/// This is an opinionated concrete implementation of a client for the Model Control Protocol (MCP).
/// I contrast to the other services, it does not expose a generic trait interface.
///
/// It is designed to be trivially cloneable.
#[derive(Clone)]
pub struct McpClient {
    pub inner: Arc<McpClientInner>,
}

impl Deref for McpClient {
    type Target = McpClientInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Inner implementation of the MCP client.
pub struct McpClientInner {
    pub mcps: Vec<Mcp>,
}

impl McpClient {
    /// Creates a new MCP client.
    pub async fn new(path: &str) -> Res<Self> {
        // Load the MCP JSON configuration.
        let json_servers = load_mcp_json(path);

        // Parse the JSON into a vector of `McpServer`.
        let servers = get_servers_from_mcp_json(json_servers)?;

        // Get the tools from the MCP servers.
        let mcps = hydrate_mcps(servers.iter()).await?;

        // Create the inner MCP client.
        let inner = Arc::new(McpClientInner { mcps });

        Ok(Self { inner })
    }

    /// Returns a reference to the inner MCP client.
    pub fn inner(&self) -> &McpClientInner {
        &self.inner
    }
}

// Helpers.

/// Load the MCP JSON configuration from the given path.
pub fn load_mcp_json(path: &str) -> Map<String, Value> {
    // Load the `mcp.json` from the configuration.
    let json = std::fs::read_to_string(path).unwrap_or("{}".to_string());
    let mut json = serde_json::from_str::<Value>(&json).unwrap_or_else(|_| serde_json::json!({}));

    let mut json_servers = json.get("servers").unwrap_or(&serde_json::json!({})).clone().as_object().unwrap().clone();
    if let Some(json_cursor_servers) = json.get_mut("mcpServers") {
        json_servers.append(json_cursor_servers.as_object_mut().unwrap());
    };

    json_servers
}

/// Load the MCP JSON configuration into memory.
pub fn get_servers_from_mcp_json(json_servers: Map<String, Value>) -> Res<Vec<McpServer>> {
    dbg!(json_servers)
        .into_iter()
        .map(|(name, value)| {
            dbg!(name.clone());
            dbg!(&value);
            let config = serde_json::from_value::<McpServerConfig>(value)?;
            Ok(McpServer { name, config })
        })
        .collect::<Res<Vec<_>>>()
}

/// Given an [`McpServer`], get the tools.
pub async fn get_mcp_server_client(server: &McpServer) -> Res<RunningService<RoleClient, ()>> {
    match &server.config {
        McpServerConfig::Local { command, args, envs } => {
            let mut command = Command::new(command);

            command.args(args);

            if let Some(envs) = envs {
                for (key, value) in envs {
                    command.env(key, value);
                }
            }

            let transport = TokioChildProcess::new(command)?;

            Ok(().serve(transport).await?)
        }
        McpServerConfig::Remote { url, headers } => {
            // Compute headers.
            let mut header_map = HeaderMap::new();
            if let Some(headers_vec) = headers {
                for (key, value) in headers_vec {
                    header_map.insert(HeaderName::from_str(key)?, HeaderValue::from_str(value)?);
                }
            }

            // Build client.
            let client = reqwest::Client::builder().default_headers(header_map).build()?;

            // Build config.
            let config = StreamableHttpClientTransportConfig::with_uri(url.as_str());

            // Build the transport.
            let transport = StreamableHttpClientTransport::with_client(client, config);

            Ok(().serve(transport).await?)
        }
    }
}

/// Get the tools from the MCP server.
pub async fn hydrate_mcps(servers: impl IntoIterator<Item = &McpServer>) -> Res<Vec<Mcp>> {
    // For each server, enumerate its tools, and create a `RunningService` for each.
    let tools_tasks = servers
        .into_iter()
        .map(|server| async move {
            let client = Arc::new(get_mcp_server_client(server).await?);
            let tools = client.list_all_tools().await?;

            Ok(Mcp { name: server.name.clone(), client, tools })
        })
        .collect::<Vec<_>>();

    // Run all tasks concurrently and collect the results.
    let mcps = futures::future::join_all(tools_tasks).await.into_iter().collect::<Res<Vec<_>>>()?;

    Ok(mcps)
}

// Tests.

#[cfg(test)]
mod tests {
    use rmcp::model::CallToolRequestParam;
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn test_get_mcp_server_tools_local() {
        let server = McpServer {
            name: "server-everything".into(),
            config: McpServerConfig::Local {
                command: "npx".into(),
                args: vec!["-y".into(), "@modelcontextprotocol/server-everything".into()],
                envs: None,
            },
        };

        let client = get_mcp_server_client(&server).await.unwrap();
        let tools = client.list_all_tools().await.unwrap();

        assert_eq!(tools.len(), 8);
        assert_eq!(tools[0].name, "echo");
    }

    #[tokio::test]
    async fn test_get_mcp_server_tools_remote() {
        let server = McpServer {
            name: "deepwiki".into(),
            config: McpServerConfig::Remote {
                url: "https://mcp.deepwiki.com/mcp".into(),
                headers: None,
            },
        };

        let client = get_mcp_server_client(&server).await.unwrap();
        let tools = client.list_all_tools().await.unwrap();

        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0].name, "read_wiki_structure");
        assert_eq!(tools[1].name, "read_wiki_contents");
        assert_eq!(tools[2].name, "ask_question");
    }

    #[tokio::test]
    async fn test_call_tool_local() {
        let server = McpServer {
            name: "server-everything".into(),
            config: McpServerConfig::Local {
                command: "npx".into(),
                args: vec!["-y".into(), "@modelcontextprotocol/server-everything".into()],
                envs: None,
            },
        };

        let client = get_mcp_server_client(&server).await.unwrap();
        let request = CallToolRequestParam {
            name: "echo".into(),
            arguments: Some(
                json!({
                    "message": "Hello, MCP!"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        };

        let result = client.call_tool(request).await.unwrap();

        assert_eq!(result.content[0].as_text().unwrap().text, "Echo: Hello, MCP!");
    }

    #[tokio::test]
    async fn test_call_tool_remote() {
        let server = McpServer {
            name: "deepwiki".into(),
            config: McpServerConfig::Remote {
                url: "https://mcp.deepwiki.com/mcp".into(),
                headers: None,
            },
        };

        let client = get_mcp_server_client(&server).await.unwrap();
        let request = CallToolRequestParam {
            name: "read_wiki_structure".into(),
            arguments: Some(
                json!({
                    "repoName": "twitchax/triage-bot",
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        };

        let result = client.call_tool(request).await.unwrap();

        assert!(!result.content[0].as_text().unwrap().text.is_empty());
    }

    #[test]
    fn test_load_mcp_json() {
        let json = load_mcp_json("tests/mcp.json");

        assert_eq!(json.len(), 1);
        assert!(json.contains_key("everything"));

        let everything = json.get("everything").unwrap();
        assert_eq!(everything["command"], "npx");
        assert_eq!(everything["args"], json!(["-y", "@modelcontextprotocol/server-everything"]));
    }

    #[test]
    fn test_get_servers_from_mcp_json() {
        let json = load_mcp_json("tests/mcp.json");
        let servers = get_servers_from_mcp_json(json).unwrap();

        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "everything");
    }

    #[tokio::test]
    async fn test_create_mcp_client() {
        let client = McpClient::new("tests/mcp.json").await.unwrap();

        assert!(!client.mcps.is_empty());

        let everything_mcp = client.mcps.iter().find(|mcp| mcp.name == "everything").unwrap();

        assert_eq!(everything_mcp.name, "everything");
        assert_eq!(everything_mcp.tools[0].name, "echo");

        let request = CallToolRequestParam {
            name: "echo".into(),
            arguments: Some(
                json!({
                    "message": "Hello, MCP!"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        };
        let result = everything_mcp.client.call_tool(request).await.unwrap();
        assert_eq!(result.content[0].as_text().unwrap().text, "Echo: Hello, MCP!");
    }
}
