use anyhow::Result;
use clap::Parser;
use rmcp::{ServiceExt, transport::stdio};
use std::path::PathBuf;
use tracing_subscriber::{self, filter::EnvFilter};

use ast_grep_mcp::{ast_grep_service::AstGrepService, config::ServiceConfig};

/// AST-Grep MCP Server - Structural code search and transformation
#[derive(Parser, Debug)]
#[command(name = "ast-grep-mcp")]
#[command(about = "Model Context Protocol server for ast-grep")]
#[command(version)]
struct Args {
    /// Root directories to search in (can be specified multiple times)
    #[arg(
        short = 'd',
        long = "root-dir",
        help = "Root directory to search in (default: current directory)",
        value_name = "PATH"
    )]
    root_directories: Vec<PathBuf>,

    /// Maximum file size to process (in bytes)
    #[arg(
        long = "max-file-size",
        default_value = "52428800", // 50MB
        help = "Maximum file size to process in bytes"
    )]
    max_file_size: u64,

    /// Maximum number of concurrent file operations
    #[arg(
        long = "max-concurrency",
        default_value = "10",
        help = "Maximum number of concurrent file operations"
    )]
    max_concurrency: usize,

    /// Maximum number of results to return per search
    #[arg(
        long = "limit",
        default_value = "1000",
        help = "Maximum number of results to return per search"
    )]
    limit: usize,

    /// Directory for storing custom rules
    #[arg(
        long = "rules-dir",
        help = "Directory for storing custom rules (default: ~/.ast-grep-mcp/rules)",
        value_name = "PATH"
    )]
    rules_directory: Option<PathBuf>,

    /// Maximum number of compiled patterns to cache
    #[arg(
        long = "pattern-cache-size",
        default_value = "1000",
        help = "Maximum number of compiled patterns to cache"
    )]
    pattern_cache_size: usize,
}

#[tokio::main]
#[tracing::instrument]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize the tracing subscriber with file and stdout logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting MCP server with config: {:?}", args);

    // Create a custom config from command line arguments
    let config = create_config_from_args(args)?;

    // Create an instance of our ast-grep service with custom config
    let service = AstGrepService::with_config(config).serve(stdio()).await?;

    tracing::info!("Service started, waiting for connections");
    service.waiting().await?;
    Ok(())
}

/// Create a ServiceConfig from command line arguments
fn create_config_from_args(args: Args) -> Result<ServiceConfig> {
    let root_directories = if args.root_directories.is_empty() {
        // Default to current working directory
        vec![std::env::current_dir()?]
    } else {
        args.root_directories
    };

    let rules_directory = args.rules_directory.unwrap_or_else(|| {
        // Default to ~/.ast-grep-mcp/rules
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ast-grep-mcp")
            .join("rules")
    });

    Ok(ServiceConfig {
        max_file_size: args.max_file_size,
        max_concurrency: args.max_concurrency,
        limit: args.limit,
        root_directories,
        rules_directory,
        pattern_cache_size: args.pattern_cache_size,
    })
}
