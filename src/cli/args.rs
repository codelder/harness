use clap::Parser;

/// AI Agent Harness - A high-performance agent CLI
#[derive(Parser, Debug)]
#[command(name = "harness")]
#[command(version = "0.1.0")]
#[command(about = "High-performance, memory-safe AI agent harness", long_about = None)]
pub struct Args {
    /// LLM provider to use
    #[arg(short, long, value_enum, default_value = "anthropic")]
    pub provider: Provider,

    /// Model to use (provider-specific)
    #[arg(short, long, default_value = "claude-3-5-sonnet-20241022")]
    pub model: String,

    /// Base URL for the API (optional, for proxies or custom endpoints)
    /// For Anthropic: can also set ANTHROPIC_BASE_URL env var
    /// For OpenAI: can also set OPENAI_BASE_URL env var
    #[arg(long)]
    pub base_url: Option<String>,

    /// Verbosity level (-v for info, -vv for debug)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

/// Supported LLM providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Provider {
    Anthropic,
    Openai,
    Ollama,
}

impl Args {
    /// Convert verbose flag count to tracing level
    pub fn tracing_level(&self) -> tracing::Level {
        match self.verbose {
            0 => tracing::Level::WARN,
            1 => tracing::Level::INFO,
            _ => tracing::Level::DEBUG,
        }
    }

    /// Get default model for a provider
    pub fn default_model(provider: Provider) -> &'static str {
        match provider {
            Provider::Anthropic => "claude-3-5-sonnet-20241022",
            Provider::Openai => "gpt-4o",
            Provider::Ollama => "llama3.2",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_level_warn() {
        let args = Args {
            provider: Provider::Anthropic,
            model: "claude-3-5-sonnet".to_string(),
            base_url: None,
            verbose: 0,
        };
        assert_eq!(args.tracing_level(), tracing::Level::WARN);
    }

    #[test]
    fn test_tracing_level_info() {
        let args = Args {
            provider: Provider::Anthropic,
            model: "claude-3-5-sonnet".to_string(),
            base_url: None,
            verbose: 1,
        };
        assert_eq!(args.tracing_level(), tracing::Level::INFO);
    }

    #[test]
    fn test_tracing_level_debug() {
        let args = Args {
            provider: Provider::Anthropic,
            model: "claude-3-5-sonnet".to_string(),
            base_url: None,
            verbose: 2,
        };
        assert_eq!(args.tracing_level(), tracing::Level::DEBUG);
    }

    #[test]
    fn test_default_models() {
        assert_eq!(Args::default_model(Provider::Anthropic), "claude-3-5-sonnet-20241022");
        assert_eq!(Args::default_model(Provider::Openai), "gpt-4o");
        assert_eq!(Args::default_model(Provider::Ollama), "llama3.2");
    }
}
