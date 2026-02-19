//! MCP Server for propel
//!
//! MCP Protocol (stdio) <-> propel-cloud / propel-core / propel-build
//!
//! Each tool is a thin wrapper around existing CLI logic.
//! DoctorReport formatting (`Display` impl) is shared with the CLI.

use anyhow::Result;
use clap::Args;
use propel_build::dockerfile::DockerfileGenerator;
use propel_build::{bundle, eject as eject_mod};
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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// Map any `Display` error to an MCP internal error.
fn internal_err(e: impl std::fmt::Display) -> McpError {
    McpError::internal_error(format!("{e}"), None)
}

// =============================================================================
// CLI entry point
// =============================================================================

/// MCP subcommand arguments
#[derive(Args)]
#[command(after_long_help = r#"SETUP (Claude Code .mcp.json):
  {
    "mcpServers": {
      "propel": {
        "command": "propel",
        "args": ["mcp"]
      }
    }
  }

The project path is auto-detected via MCP roots protocol.
Use -p only when the client does not support roots.

TOOLS PROVIDED:
  doctor, status, logs, secret_list, config, deploy, eject

EXAMPLES:
  $ propel mcp                   # auto-detect from MCP roots
  $ propel mcp -p ./my-project   # explicit fallback
"#)]
pub(crate) struct McpArgs {
    /// Fallback project path (auto-detected from MCP roots when omitted)
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

/// Execute the MCP server
pub(crate) async fn execute(args: McpArgs) -> Result<()> {
    run_mcp_server(args).await
}

async fn run_mcp_server(args: McpArgs) -> Result<()> {
    let cli_path = match args.path {
        Some(p) => Some(p.canonicalize().map_err(|e| {
            anyhow::anyhow!("Project path '{}' not accessible: {e}", p.display())
        })?),
        None => None,
    };

    let server = PropelMcpServer::new(cli_path);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}

// =============================================================================
// MCP Server
// =============================================================================

#[derive(Clone)]
struct PropelMcpServer {
    /// Fallback path from `-p` flag (used when roots protocol is unavailable).
    cli_path: Option<PathBuf>,
    /// Resolved project path (from roots protocol or cli_path).
    resolved_path: Arc<OnceCell<PathBuf>>,
    tool_router: ToolRouter<Self>,
}

impl PropelMcpServer {
    fn new(cli_path: Option<PathBuf>) -> Self {
        Self {
            cli_path,
            resolved_path: Arc::new(OnceCell::new()),
            tool_router: Self::tool_router(),
        }
    }

    /// Resolve project path: roots protocol first, then `-p` fallback.
    async fn project_path(
        &self,
        peer: &rmcp::service::Peer<RoleServer>,
    ) -> Result<PathBuf, McpError> {
        let path = self
            .resolved_path
            .get_or_try_init(|| async {
                // Try MCP roots protocol
                if let Ok(result) = peer.list_roots().await
                    && let Some(root) = result.roots.first()
                    && let Some(path) = root.uri.strip_prefix("file://")
                {
                    let p = PathBuf::from(path);
                    if p.exists() {
                        return Ok(p);
                    }
                }

                // Fallback to CLI -p flag
                if let Some(ref p) = self.cli_path {
                    return Ok(p.clone());
                }

                Err(McpError::internal_error(
                    "Project path not available. \
                     The MCP client did not provide roots, and no -p flag was given."
                        .to_string(),
                    None,
                ))
            })
            .await?;
        Ok(path.clone())
    }

    fn load_config(project_path: &Path) -> Result<PropelConfig, McpError> {
        PropelConfig::load(project_path)
            .map_err(|e| McpError::internal_error(format!("Failed to load config: {e}"), None))
    }

    fn load_meta(project_path: &Path) -> Result<ProjectMeta, McpError> {
        ProjectMeta::from_cargo_toml(project_path).map_err(|e| {
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
        super::service_name(config, meta)
    }

    /// Determine Dockerfile and bundle source into a temp directory.
    fn prepare_bundle(
        project_path: &Path,
        config: &PropelConfig,
        meta: &ProjectMeta,
        steps: &mut Vec<String>,
    ) -> Result<PathBuf, McpError> {
        let dockerfile_content = if eject_mod::is_ejected(project_path) {
            steps.push("Using ejected Dockerfile".to_string());
            eject_mod::load_ejected_dockerfile(project_path).map_err(internal_err)?
        } else {
            let generator = DockerfileGenerator::new(&config.build, meta, config.cloud_run.port);
            generator.render()
        };

        let bundle_dir =
            bundle::create_bundle(project_path, &dockerfile_content).map_err(internal_err)?;
        steps.push("Source bundled".to_string());
        Ok(bundle_dir)
    }

    /// Discover secrets in Secret Manager (non-fatal on failure).
    async fn discover_secrets(
        project_id: &str,
        client: &GcloudClient,
        steps: &mut Vec<String>,
    ) -> Vec<String> {
        match client.list_secrets(project_id).await {
            Ok(s) => {
                if s.is_empty() {
                    steps.push("No secrets found in Secret Manager".to_string());
                } else {
                    steps.push(format!("{} secret(s) will be injected", s.len()));
                }
                s
            }
            Err(e) => {
                steps.push(format!("Warning: could not list secrets: {e}"));
                vec![]
            }
        }
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
        peer: rmcp::service::Peer<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let project_path = self.project_path(&peer).await?;

        // Intentionally loads config without `Self::load_config()?`:
        // doctor must report diagnostics even when propel.toml is missing or invalid.
        let config = PropelConfig::load(&project_path);
        let project_id = config
            .as_ref()
            .ok()
            .and_then(|c| c.project.gcp_project_id.as_deref());

        let client = GcloudClient::new();
        let mut report = client.doctor(project_id).await;

        // Config file check
        if project_path.join("propel.toml").exists() {
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
        peer: rmcp::service::Peer<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let project_path = self.project_path(&peer).await?;
        let config = Self::load_config(&project_path)?;
        let meta = Self::load_meta(&project_path)?;
        let project_id = Self::require_project_id(&config)?;
        let service_name = Self::service_name(&config, &meta);

        let client = GcloudClient::new();
        let output = client
            .describe_service(service_name, project_id, &config.project.region)
            .await
            .map_err(internal_err)?;

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
        peer: rmcp::service::Peer<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let project_path = self.project_path(&peer).await?;
        let config = Self::load_config(&project_path)?;
        let meta = Self::load_meta(&project_path)?;
        let project_id = Self::require_project_id(&config)?;
        let service_name = Self::service_name(&config, &meta);

        let limit = req.tail.unwrap_or(100).min(1000);

        let client = GcloudClient::new();
        let output = client
            .read_logs_captured(service_name, project_id, &config.project.region, limit)
            .await
            .map_err(internal_err)?;

        let text = if output.trim().is_empty() {
            "No log entries found".to_string()
        } else {
            output
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
        peer: rmcp::service::Peer<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let project_path = self.project_path(&peer).await?;
        let config = Self::load_config(&project_path)?;
        let project_id = Self::require_project_id(&config)?;

        let client = GcloudClient::new();
        let secrets = client
            .list_secrets(project_id)
            .await
            .map_err(internal_err)?;

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
        peer: rmcp::service::Peer<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let project_path = self.project_path(&peer).await?;
        let config = Self::load_config(&project_path)?;

        let json = serde_json::to_string_pretty(&config).map_err(internal_err)?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

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
        peer: rmcp::service::Peer<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let project_path = self.project_path(&peer).await?;
        let client = GcloudClient::new();
        let mut steps = Vec::new();

        // Dirty check
        if !req.allow_dirty && bundle::is_dirty(&project_path).map_err(internal_err)? {
            return Err(McpError::invalid_request(
                "Uncommitted changes detected. \
                 Commit your changes, or set allow_dirty=true to deploy anyway."
                    .to_string(),
                None,
            ));
        }

        // Load configuration
        let config = Self::load_config(&project_path)?;
        let meta = Self::load_meta(&project_path)?;
        let gcp_project_id = Self::require_project_id(&config)?;
        let service_name = Self::service_name(&config, &meta);
        let region = &config.project.region;
        let image_tag = format!(
            "{}:latest",
            super::image_path(
                region,
                gcp_project_id,
                super::ARTIFACT_REPO_NAME,
                service_name
            ),
        );

        // Pre-flight checks
        let report = client
            .check_prerequisites(gcp_project_id)
            .await
            .map_err(internal_err)?;
        if report.has_warnings() {
            let disabled = report.disabled_apis.join(", ");
            return Err(McpError::internal_error(
                format!(
                    "Required APIs not enabled: {disabled}. \
                     Enable them with: gcloud services enable <api> --project {gcp_project_id}"
                ),
                None,
            ));
        }
        steps.push("Pre-flight checks passed".to_string());

        // Ensure Artifact Registry repository
        client
            .ensure_artifact_repo(gcp_project_id, region, super::ARTIFACT_REPO_NAME)
            .await
            .map_err(internal_err)?;
        steps.push("Artifact Registry repository ensured".to_string());

        // Bundle source
        let bundle_dir = Self::prepare_bundle(&project_path, &config, &meta, &mut steps)?;

        // Submit build (captured for MCP response)
        let build_output = client
            .submit_build_captured(&bundle_dir, gcp_project_id, &image_tag)
            .await
            .map_err(internal_err)?;
        steps.push("Cloud Build completed".to_string());

        // Discover secrets & deploy to Cloud Run
        let secrets = Self::discover_secrets(gcp_project_id, &client, &mut steps).await;
        let url = client
            .deploy_to_cloud_run(
                service_name,
                &image_tag,
                gcp_project_id,
                region,
                &config.cloud_run,
                &secrets,
            )
            .await
            .map_err(internal_err)?;
        steps.push(format!("Deployed: {url}"));

        // Format response
        let mut text = steps.join("\n");
        if !build_output.is_empty() {
            text.push_str(&format!("\n\n--- Cloud Build Log ---\n{build_output}"));
        }

        Ok(CallToolResult::success(vec![Content::text(text)]))
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
        peer: rmcp::service::Peer<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let project_path = self.project_path(&peer).await?;
        let config = Self::load_config(&project_path)?;
        let meta = Self::load_meta(&project_path)?;

        let generator = DockerfileGenerator::new(&config.build, &meta, config.cloud_run.port);
        let dockerfile = generator.render();

        eject_mod::eject(&project_path, &dockerfile).map_err(internal_err)?;

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
        let server = PropelMcpServer::new(Some(PathBuf::from(".")));
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
