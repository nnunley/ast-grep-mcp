use anyhow::Result;
use clap::{Parser, Subcommand};
use rmcp::{ServiceExt, transport::stdio};
use std::path::PathBuf;
use tracing_subscriber::{self, filter::EnvFilter};

use ast_grep_mcp::{
    GenerateAstParam, RuleReplaceParam, RuleSearchParam, SearchParam,
    ast_grep_service::AstGrepService, config::ServiceConfig, types::*,
};

/// AST-Grep MCP Server - Structural code search and transformation
#[derive(Parser, Debug)]
#[command(name = "ast-grep-mcp")]
#[command(about = "Model Context Protocol server for ast-grep with CLI testing support")]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    #[command(flatten)]
    global: GlobalArgs,
}

#[derive(Parser, Debug)]
struct GlobalArgs {
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

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start MCP server (default mode)
    Serve,
    /// Search for patterns in code
    Search {
        /// Pattern to search for
        #[arg(short, long)]
        pattern: String,
        /// Programming language
        #[arg(short, long)]
        language: String,
        /// Code to search in (use - for stdin)
        #[arg(long)]
        code: Option<String>,
        /// File to search in
        #[arg(short, long)]
        file: Option<PathBuf>,
    },
    /// Search files using ast-grep patterns
    FileSearch {
        /// Pattern to search for
        #[arg(short, long)]
        pattern: String,
        /// Programming language
        #[arg(short, long)]
        language: String,
        /// Path pattern (glob)
        #[arg(long, default_value = "**/*")]
        path_pattern: String,
        /// Maximum results
        #[arg(long, default_value = "100")]
        max_results: usize,
    },
    /// Search files using rules
    RuleSearch {
        /// Rule file path
        #[arg(short, long)]
        rule: PathBuf,
        /// Path pattern (glob)
        #[arg(long)]
        path_pattern: Option<String>,
        /// Maximum results
        #[arg(long, default_value = "100")]
        max_results: usize,
    },
    /// Replace patterns in files using rules
    RuleReplace {
        /// Rule file path
        #[arg(short, long)]
        rule: PathBuf,
        /// Path pattern (glob)
        #[arg(long)]
        path_pattern: Option<String>,
        /// Actually modify files (default: dry run)
        #[arg(long)]
        apply: bool,
        /// Show summary only
        #[arg(long)]
        summary_only: bool,
        /// Maximum results
        #[arg(long, default_value = "100")]
        max_results: usize,
    },
    /// Generate AST for code
    GenerateAst {
        /// Programming language
        #[arg(short, long)]
        language: String,
        /// Code to analyze (use - for stdin)
        #[arg(long)]
        code: Option<String>,
        /// File to analyze
        #[arg(short, long)]
        file: Option<PathBuf>,
    },
}

#[tokio::main]
#[tracing::instrument]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize the tracing subscriber based on mode
    let log_level = if matches!(args.command, Some(Commands::Serve) | None) {
        tracing::Level::DEBUG
    } else {
        tracing::Level::WARN // Less verbose for CLI commands
    };

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(log_level.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    // Create a custom config from command line arguments
    let config = create_config_from_args(args.global)?;

    match args.command {
        Some(Commands::Serve) | None => {
            // Default MCP server mode
            tracing::info!("Starting MCP server with config: {:?}", config);
            let service = AstGrepService::with_config(config).serve(stdio()).await?;
            tracing::info!("Service started, waiting for connections");
            service.waiting().await?;
        }
        Some(command) => {
            // CLI command mode
            run_cli_command(command, config).await?;
        }
    }

    Ok(())
}

/// Create a ServiceConfig from command line arguments
fn create_config_from_args(args: GlobalArgs) -> Result<ServiceConfig> {
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

/// Run CLI commands for testing and debugging
async fn run_cli_command(command: Commands, config: ServiceConfig) -> Result<()> {
    let service = AstGrepService::with_config(config);

    match command {
        Commands::Serve => unreachable!(), // Handled in main

        Commands::Search {
            pattern,
            language,
            code,
            file,
        } => {
            let code_content = get_code_content(code, file).await?;
            let param = SearchParam {
                code: code_content,
                pattern,
                language,
            };

            let result = service.search(param).await?;
            println!("Found {} matches:", result.matches.len());
            for (i, match_result) in result.matches.iter().enumerate() {
                println!(
                    "Match {}: {}:{}-{}:{}",
                    i + 1,
                    match_result.start_line,
                    match_result.start_col,
                    match_result.end_line,
                    match_result.end_col
                );
                println!("  Text: {}", match_result.text);
            }
        }

        Commands::FileSearch {
            pattern,
            language,
            path_pattern,
            max_results,
        } => {
            let param = FileSearchParam {
                pattern,
                language,
                path_pattern,
                max_results,
                max_file_size: 1024 * 1024, // 1MB default
                cursor: None,
            };

            let result = service.file_search(param).await?;
            println!("Found matches in {} files:", result.matches.len());
            for file_match in &result.matches {
                println!(
                    "File: {} ({} matches)",
                    file_match.file_path,
                    file_match.matches.len()
                );
                for (i, match_result) in file_match.matches.iter().enumerate() {
                    println!(
                        "  Match {}: {}:{}-{}:{}",
                        i + 1,
                        match_result.start_line,
                        match_result.start_col,
                        match_result.end_line,
                        match_result.end_col
                    );
                    println!("    Text: {}", match_result.text.trim());
                }
            }
        }

        Commands::RuleSearch {
            rule,
            path_pattern,
            max_results,
        } => {
            let rule_config = std::fs::read_to_string(&rule)?;
            let param = RuleSearchParam {
                rule_config,
                path_pattern,
                max_results,
                max_file_size: 1024 * 1024, // 1MB default
                cursor: None,
            };

            let result = service.rule_search(param).await?;
            println!(
                "Rule search found matches in {} files:",
                result.matches.len()
            );
            for file_match in &result.matches {
                println!(
                    "File: {} ({} matches)",
                    file_match.file_path,
                    file_match.matches.len()
                );
                for (i, match_result) in file_match.matches.iter().enumerate() {
                    println!(
                        "  Match {}: {}:{}-{}:{}",
                        i + 1,
                        match_result.start_line,
                        match_result.start_col,
                        match_result.end_line,
                        match_result.end_col
                    );
                    println!("    Text: {}", match_result.text.trim());
                }
            }
        }

        Commands::RuleReplace {
            rule,
            path_pattern,
            apply,
            summary_only,
            max_results,
        } => {
            let rule_config = std::fs::read_to_string(&rule)?;
            let param = RuleReplaceParam {
                rule_config,
                path_pattern,
                max_results,
                max_file_size: 1024 * 1024, // 1MB default
                dry_run: !apply,            // Invert apply flag
                summary_only,
                cursor: None,
            };

            let result = service.rule_replace(param).await?;

            if apply {
                println!("Applied changes to {} files:", result.files_with_changes);
            } else {
                println!(
                    "DRY RUN - Would modify {} files:",
                    result.files_with_changes
                );
            }

            println!("Total changes: {}", result.total_changes);

            if !summary_only {
                for file_result in &result.file_results {
                    println!("\nFile: {}", file_result.file_path);
                    println!("Changes: {}", file_result.total_changes);
                    for (i, change) in file_result.changes.iter().enumerate() {
                        println!("  Change {}: Line {}", i + 1, change.start_line);
                        println!("    - {}", change.old_text.trim());
                        println!("    + {}", change.new_text.trim());
                    }
                }
            }
        }

        Commands::GenerateAst {
            language,
            code,
            file,
        } => {
            let code_content = get_code_content(code, file).await?;
            let param = GenerateAstParam {
                code: code_content,
                language,
            };

            let result = service.generate_ast(param).await?;
            println!("Language: {}", result.language);
            println!("Code length: {} characters", result.code_length);
            println!("Available node kinds: {}", result.node_kinds.join(", "));
            println!("\nAST structure:");
            println!("{}", result.ast);
        }
    }

    Ok(())
}

/// Get code content from either direct input, file, or stdin
async fn get_code_content(code: Option<String>, file: Option<PathBuf>) -> Result<String> {
    match (code, file) {
        (Some(code), None) => {
            if code == "-" {
                // Read from stdin
                use tokio::io::{self, AsyncReadExt};
                let mut stdin = io::stdin();
                let mut buffer = String::new();
                stdin.read_to_string(&mut buffer).await?;
                Ok(buffer)
            } else {
                Ok(code)
            }
        }
        (None, Some(file)) => Ok(std::fs::read_to_string(file)?),
        (Some(_), Some(_)) => {
            anyhow::bail!("Cannot specify both --code and --file");
        }
        (None, None) => {
            anyhow::bail!("Must specify either --code or --file");
        }
    }
}
