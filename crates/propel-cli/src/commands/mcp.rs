//! MCP Server for propel
//!
//! MCP Protocol (stdio) <-> propel-cloud / propel-core / propel-build
//!
//! Each tool is a thin wrapper around existing CLI logic.
//! Deploy pipeline and doctor formatting are shared with the CLI.

use anyhow::Result;
use clap::Args;
use propel_build::dockerfile::DockerfileGenerator;
use propel_build::eject as eject_mod;
use propel_cloud::GcloudClient;
use propel_core::{ProjectMeta, PropelConfig};
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{tool::ToolCallContext, tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
        PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    service::{RequestContext, RoleServer},
    tool, tool_router,
    transport::stdio,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// =============================================================================
// CLI entry point
// =============================================================================

/// MCP subcommand arguments
#[derive(Args)]
#[command(after_long_help = r#"SETUP (Claude Code ~/.claude.json):
  {
    "mcpServers": {
      "propel": {
        "command": "propel",
        "args": ["mcp", "-p", "/path/to/project"]
      }
    }
  }

TOOLS PROVIDED:
  doctor, status, logs, secret_list, config, deploy, eject

EXAMPLES:
  $ propel mcp -p ./my-project
"#)]
pub(crate) struct McpArgs {
    /// Project path containing propel.toml
    #[arg(short, long, default_value = ".")]
    pub path: PathBuf,
}

/// Execute the MCP server
pub(crate) async fn execute(args: McpArgs) -> Result<()> {
    run_mcp_server(args).await
}

async fn run_mcp_server(args: McpArgs) -> Result<()> {
    let project_path = args.path.canonicalize().map_err(|e| {
        anyhow::anyhow!("Project path '{}' not accessible: {e}", args.path.display())
    })?;

    let server = PropelMcpServer::new(project_path);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}

// =============================================================================
// MCP Server
// =============================================================================

#[derive(Clone)]
struct PropelMcpServer {
    project_path: PathBuf,
    tool_router: ToolRouter<Self>,
}

impl PropelMcpServer {
    fn new(project_path: PathBuf) -> Self {
        Self {
            project_path,
            tool_router: Self::tool_router(),
        }
    }

    fn load_config(&self) -> Result<PropelConfig, McpError> {
        PropelConfig::load(&self.project_path)
            .map_err(|e| McpError::internal_error(format!("Failed to load config: {e}"), None))
    }

    fn load_meta(&self) -> Result<ProjectMeta, McpError> {
        ProjectMeta::from_cargo_toml(&self.project_path).map_err(|e| {
            McpError::internal_error(format!("Failed to load project meta: {e}"), None)
        })
    }

    fn require_project_id(config: &PropelConfig) -> Result<&str, McpError> {
        config.project.gcp_project_id.as_deref().ok_or_else(|| {
            McpError::invalid_request(
                "gcp_project_id not set in propel.toml — set [project].gcp_project_id".to_string(),
                None,
            )
        })
    }

    fn service_name<'a>(config: &'a PropelConfig, meta: &'a ProjectMeta) -> &'a str {
        config.project.name.as_deref().unwrap_or(&meta.name)
    }
}

// =============================================================================
// ServerHandler impl
// =============================================================================

impl ServerHandler for PropelMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_03_26,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "propel".to_string(),
                title: Some("Propel — Deploy Rust to Cloud Run".to_string()),
                description: Some(
                    "Deploy Rust/Axum applications to Google Cloud Run with zero configuration."
                        .to_string(),
                ),
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Propel MCP server for deploying Rust apps to Cloud Run. \
                 Start with `doctor` to check GCP readiness, use `config` to see project settings, \
                 `status` to check running service, and `deploy` to build & deploy."
                    .to_string(),
            ),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: self.tool_router.list_all(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let tool_ctx = ToolCallContext::new(self, request, context);
        self.tool_router.call(tool_ctx).await
    }
}

// =============================================================================
// Request types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct McpDoctorRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct McpStatusRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct McpLogsRequest {
    #[schemars(description = "Number of log entries to return (default: 100, max: 1000)")]
    pub tail: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct McpSecretListRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct McpConfigRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct McpDeployRequest {
    #[schemars(description = "Allow deploying with uncommitted changes (default: false)")]
    #[serde(default)]
    pub allow_dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct McpEjectRequest {}

// =============================================================================
// Tool implementations — thin wrappers only
// =============================================================================

#[tool_router]
impl PropelMcpServer {
    /// Uses shared DoctorReport Display impl — no formatting duplication.
    #[tool(
        name = "doctor",
        description = "Check GCP setup and readiness for deployment. Verifies gcloud CLI, authentication, project, billing, required APIs, and propel.toml existence.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = true
        )
    )]
    async fn doctor(
        &self,
        #[allow(unused_variables)] Parameters(_req): Parameters<McpDoctorRequest>,
    ) -> Result<CallToolResult, McpError> {
        let config = PropelConfig::load(&self.project_path);
        let project_id = config
            .as_ref()
            .ok()
            .and_then(|c| c.project.gcp_project_id.as_deref());

        let client = GcloudClient::new();
        let mut report = client.doctor(project_id).await;

        // Config file check
        if self.project_path.join("propel.toml").exists() {
            report.config_file = propel_cloud::CheckResult::ok("Found");
        } else {
            report.config_file = propel_cloud::CheckResult::fail("Not found");
        }

        Ok(CallToolResult::success(vec![Content::text(
            report.to_string(),
        )]))
    }

    #[tool(
        name = "status",
        description = "Show the current Cloud Run service status (YAML format). Requires gcp_project_id in propel.toml.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = true
        )
    )]
    async fn status(
        &self,
        #[allow(unused_variables)] Parameters(_req): Parameters<McpStatusRequest>,
    ) -> Result<CallToolResult, McpError> {
        let config = self.load_config()?;
        let meta = self.load_meta()?;
        let project_id = Self::require_project_id(&config)?;
        let service_name = Self::service_name(&config, &meta);

        let client = GcloudClient::new();
        let output = client
            .describe_service(service_name, project_id, &config.project.region)
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to get status: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        name = "logs",
        description = "Read Cloud Run service logs. Returns the most recent log entries (not streaming). Requires gcp_project_id in propel.toml.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = true
        )
    )]
    async fn logs(
        &self,
        Parameters(req): Parameters<McpLogsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let config = self.load_config()?;
        let meta = self.load_meta()?;
        let project_id = Self::require_project_id(&config)?;
        let service_name = Self::service_name(&config, &meta);

        let limit = req.tail.unwrap_or(100).min(1000);

        let client = GcloudClient::new();
        let output = client
            .read_logs(
                service_name,
                project_id,
                &config.project.region,
                limit,
                true,
            )
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to read logs: {e}"), None))?;

        let text = match output {
            Some(s) if !s.trim().is_empty() => s,
            _ => "No log entries found".to_string(),
        };

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        name = "secret_list",
        description = "List all secret names in GCP Secret Manager. Returns names only, never secret values. Requires gcp_project_id in propel.toml.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = true
        )
    )]
    async fn secret_list(
        &self,
        #[allow(unused_variables)] Parameters(_req): Parameters<McpSecretListRequest>,
    ) -> Result<CallToolResult, McpError> {
        let config = self.load_config()?;
        let project_id = Self::require_project_id(&config)?;

        let client = GcloudClient::new();
        let secrets = client
            .list_secrets(project_id)
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to list secrets: {e}"), None))?;

        let output = if secrets.is_empty() {
            "No secrets found".to_string()
        } else {
            let mut lines = vec![format!("{} secret(s):", secrets.len())];
            for name in &secrets {
                lines.push(format!("  - {name}"));
            }
            lines.join("\n")
        };

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        name = "config",
        description = "Show the current propel.toml configuration as JSON. Shows project, build, and cloud_run settings with defaults applied.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn config(
        &self,
        #[allow(unused_variables)] Parameters(_req): Parameters<McpConfigRequest>,
    ) -> Result<CallToolResult, McpError> {
        let config = self.load_config()?;

        let json = serde_json::to_string_pretty(&config).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize config: {e}"), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Uses shared deploy pipeline — no logic duplication.
    #[tool(
        name = "deploy",
        description = "Full deploy pipeline: dirty check -> bundle source -> Cloud Build -> Cloud Run. Returns the deployed service URL on success. Long-running operation (~3-10 minutes).",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn deploy(
        &self,
        Parameters(req): Parameters<McpDeployRequest>,
    ) -> Result<CallToolResult, McpError> {
        let outcome = super::deploy_pipeline::run(&self.project_path, req.allow_dirty, true)
            .await
            .map_err(|e| McpError::internal_error(format!("{e}"), None))?;

        let mut parts = vec![outcome.steps.join("\n")];
        if let Some(build_log) = outcome.build_output {
            parts.push(format!("\n--- Cloud Build Log ---\n{build_log}"));
        }

        Ok(CallToolResult::success(vec![Content::text(
            parts.join("\n"),
        )]))
    }

    #[tool(
        name = "eject",
        description = "Export the generated Dockerfile to .propel/Dockerfile for manual customization. After ejecting, `propel deploy` will use the ejected file instead of generating one.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn eject(
        &self,
        #[allow(unused_variables)] Parameters(_req): Parameters<McpEjectRequest>,
    ) -> Result<CallToolResult, McpError> {
        let config = self.load_config()?;
        let meta = self.load_meta()?;

        let generator = DockerfileGenerator::new(&config.build, &meta, config.cloud_run.port);
        let dockerfile = generator.render();

        eject_mod::eject(&self.project_path, &dockerfile)
            .map_err(|e| McpError::internal_error(format!("Eject failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            "Ejected build config to .propel/Dockerfile\n\
             You can now edit it directly. `propel deploy` will use this file.",
        )]))
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_request_default_tail() {
        let req: McpLogsRequest = serde_json::from_str("{}").unwrap();
        assert!(req.tail.is_none());
    }

    #[test]
    fn logs_request_with_tail() {
        let req: McpLogsRequest = serde_json::from_str(r#"{"tail": 50}"#).unwrap();
        assert_eq!(req.tail, Some(50));
    }

    #[test]
    fn deploy_request_default_allow_dirty() {
        let req: McpDeployRequest = serde_json::from_str("{}").unwrap();
        assert!(!req.allow_dirty);
    }

    #[test]
    fn deploy_request_allow_dirty_true() {
        let req: McpDeployRequest = serde_json::from_str(r#"{"allow_dirty": true}"#).unwrap();
        assert!(req.allow_dirty);
    }

    #[test]
    fn server_info_version() {
        let server = PropelMcpServer::new(PathBuf::from("."));
        let info = server.get_info();
        assert_eq!(info.server_info.name, "propel");
        assert!(!info.server_info.version.is_empty());
    }

    #[test]
    fn service_name_uses_config_override() {
        let mut config = PropelConfig::default();
        config.project.name = Some("custom-name".to_string());
        let meta = ProjectMeta {
            name: "cargo-name".to_string(),
            version: "0.1.0".to_string(),
            binary_name: "cargo-name".to_string(),
        };
        assert_eq!(PropelMcpServer::service_name(&config, &meta), "custom-name");
    }

    #[test]
    fn service_name_falls_back_to_cargo() {
        let config = PropelConfig::default();
        let meta = ProjectMeta {
            name: "cargo-name".to_string(),
            version: "0.1.0".to_string(),
            binary_name: "cargo-name".to_string(),
        };
        assert_eq!(PropelMcpServer::service_name(&config, &meta), "cargo-name");
    }

    #[test]
    fn require_project_id_missing() {
        let config = PropelConfig::default();
        let result = PropelMcpServer::require_project_id(&config);
        assert!(result.is_err());
    }

    #[test]
    fn require_project_id_present() {
        let mut config = PropelConfig::default();
        config.project.gcp_project_id = Some("my-project".to_string());
        let result = PropelMcpServer::require_project_id(&config);
        assert_eq!(result.unwrap(), "my-project");
    }
}
