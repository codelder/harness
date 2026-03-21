use anyhow::Result;
use clap::Parser;
use harness::{Args, Provider, Session, ProviderType};
use tracing_subscriber::prelude::*;

fn setup_tracing(level: tracing::Level) {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level.to_string())),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup tracing with verbosity level
    setup_tracing(args.tracing_level());

    tracing::info!("Starting Agent Harness v0.1.0");
    tracing::debug!("Provider: {:?}, Model: {}, Verbose: {}", args.provider, args.model, args.verbose);

    // Convert CLI Provider to LLM ProviderType
    let provider_type = match args.provider {
        Provider::Anthropic => ProviderType::Anthropic,
        Provider::Openai => ProviderType::Openai,
        Provider::Ollama => ProviderType::Ollama,
    };

    // Use default model if user specified a different provider but not model
    let model = if args.model == "claude-3-5-sonnet-20241022" && args.provider != Provider::Anthropic {
        tracing::info!("Using default model for {:?}", args.provider);
        Args::default_model(args.provider)
    } else {
        &args.model
    };

    // Create and run session
    let mut session = Session::new();

    if let Err(e) = session.run(provider_type, model, args.base_url.as_deref()).await {
        tracing::error!("Session error: {}", e);
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
