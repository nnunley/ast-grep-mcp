use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::{self, filter::EnvFilter};

use ast_grep_mcp::ast_grep_service::AstGrepService;

#[tokio::main]
#[tracing::instrument]
async fn main() -> Result<()> {
    // Initialize the tracing subscriber with file and stdout logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting MCP server");

    // Create an instance of our ast-grep service
    let service = AstGrepService::new().serve(stdio()).await?;

    tracing::info!("Service started, waiting for connections");
    service.waiting().await?;
    Ok(())
}
