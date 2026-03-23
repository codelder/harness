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
    /// For Anthropic: can also set HARNESS_ANTHROPIC_URL env var
    /// For OpenAI: can also set HARNESS_OPENAI_URL env var
    #[arg(long)]
    pub base_url: Option<String>,

    /// Verbosity level (-v for info, -vv for debug, -vvv for trace with LLM logs)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Enable extended thinking/reasoning (Anthropic only, requires supported model)
    #[arg(long)]
    pub thinking: bool,

    /// Budget tokens for extended thinking (default: 10000, only used with --thinking)
    #[arg(long, default_value = "10000")]
    pub thinking_budget: u64,
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
    /// Note: Use -vvv for trace level to enable LLM logging
    pub fn tracing_level(&self) -> tracing::Level {
        match self.verbose {
            0 => tracing::Level::WARN,
            1 => tracing::Level::INFO,
            2 => tracing::Level::DEBUG,
            _ => tracing::Level::TRACE,
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
            thinking: false,
            thinking_budget: 10000,
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
            thinking: false,
            thinking_budget: 10000,
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
            thinking: false,
            thinking_budget: 10000,
        };
        assert_eq!(args.tracing_level(), tracing::Level::DEBUG);
    }

    #[test]
    fn test_tracing_level_trace() {
        let args = Args {
            provider: Provider::Anthropic,
            model: "claude-3-5-sonnet".to_string(),
            base_url: None,
            verbose: 3,
            thinking: false,
            thinking_budget: 10000,
        };
        assert_eq!(args.tracing_level(), tracing::Level::TRACE);
    }

    #[test]
    fn test_default_models() {
        assert_eq!(Args::default_model(Provider::Anthropic), "claude-3-5-sonnet-20241022");
        assert_eq!(Args::default_model(Provider::Openai), "gpt-4o");
        assert_eq!(Args::default_model(Provider::Ollama), "llama3.2");
    }
}
