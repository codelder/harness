use anyhow::Result;
use clap::Parser;
use harness::{Args, Provider, ProviderType, Session};
use std::path::PathBuf;
use tracing_subscriber::prelude::*;
use tracing_subscriber::Layer;

fn get_log_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".harness")
}

/// Check if trace level is enabled (via CLI args or RUST_LOG env var)
fn is_trace_enabled(cli_level: tracing::Level) -> bool {
    if cli_level == tracing::Level::TRACE {
        return true;
    }
    if let Ok(rust_log) = std::env::var("RUST_LOG") {
        if rust_log.to_lowercase().contains("trace") {
            return true;
        }
    }
    false
}

/// Setup tracing with optional LLM log file (only at trace level)
/// Returns llm_guard if trace level is enabled (must be kept alive)
fn setup_tracing(level: tracing::Level) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let log_dir = get_log_dir();

    // Create log directory if it doesn't exist
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir).expect("Failed to create log directory");
    }

    // Main log file appender
    let main_appender = tracing_appender::rolling::never(&log_dir, "harness.log");
    let (main_non_blocking, main_guard) = tracing_appender::non_blocking(main_appender);

    // Only create LLM log file when trace level is enabled
    let llm_guard = if is_trace_enabled(level) {
        let llm_appender = tracing_appender::rolling::never(&log_dir, "llm.log");
        let (llm_non_blocking, guard) = tracing_appender::non_blocking(llm_appender);

        tracing_subscriber::registry()
            // Main log layer: exclude rig::completions and rig::responses
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(main_non_blocking)
                    .with_file(true)
                    .with_line_number(true)
                    .with_ansi(false)
                    .with_filter(tracing_subscriber::filter::FilterFn::new(|meta| {
                        !meta.target().starts_with("rig::completions")
                            && !meta.target().starts_with("rig::responses")
                    }))
                    .with_filter(
                        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(
                            |_| tracing_subscriber::EnvFilter::new(level.to_string()),
                        ),
                    ),
            )
            // LLM log layer: only rig::completions and rig::responses (raw LLM API logs)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(llm_non_blocking)
                    .with_file(true)
                    .with_line_number(true)
                    .with_target(true)
                    .with_ansi(false)
                    .with_filter(tracing_subscriber::filter::FilterFn::new(|meta| {
                        meta.target().starts_with("rig::completions")
                            || meta.target().starts_with("rig::responses")
                    }))
                    .with_filter(tracing_subscriber::EnvFilter::new("rig=trace")),
            )
            .init();

        Some(guard)
    } else {
        // No LLM log when not trace level
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(main_non_blocking)
                    .with_file(true)
                    .with_line_number(true)
                    .with_ansi(false)
                    .with_filter(
                        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(
                            |_| tracing_subscriber::EnvFilter::new(level.to_string()),
                        ),
                    ),
            )
            .init();

        None
    };

    // Keep main_guard alive, return llm_guard if it exists
    std::mem::forget(main_guard);
    llm_guard
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup tracing with verbosity level
    // Guards must be kept alive for the duration of the program
    let _guards = setup_tracing(args.tracing_level());

    tracing::info!("Starting Agent Harness v0.1.0");
    tracing::info!("Log file: {}/harness.log", get_log_dir().display());
    tracing::debug!(
        "Provider: {:?}, Model: {}, Verbose: {}",
        args.provider,
        args.model,
        args.verbose
    );

    // Convert CLI Provider to LLM ProviderType
    let provider_type = match args.provider {
        Provider::Anthropic => ProviderType::Anthropic,
        Provider::Openai => ProviderType::Openai,
        Provider::Ollama => ProviderType::Ollama,
    };

    // Use default model if user specified a different provider but not model
    let model =
        if args.model == "claude-3-5-sonnet-20241022" && args.provider != Provider::Anthropic {
            tracing::info!("Using default model for {:?}", args.provider);
            Args::default_model(args.provider)
        } else {
            &args.model
        };

    // Create and run session
    let mut session = Session::new();

    if let Err(e) = session
        .run(
            provider_type,
            model,
            args.base_url.as_deref(),
            args.thinking,
            args.thinking_budget,
        )
        .await
    {
        tracing::error!("Session error: {}", e);
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
