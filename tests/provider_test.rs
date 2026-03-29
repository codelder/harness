use harness::{ProviderError, ProviderType};

#[test]
fn test_parse_anthropic_provider() {
    let provider: ProviderType = "anthropic".parse().unwrap();
    assert_eq!(provider, ProviderType::Anthropic);
}

#[test]
fn test_parse_openai_provider() {
    let provider: ProviderType = "openai".parse().unwrap();
    assert_eq!(provider, ProviderType::Openai);
}

#[test]
fn test_parse_ollama_provider() {
    let provider: ProviderType = "ollama".parse().unwrap();
    assert_eq!(provider, ProviderType::Ollama);
}

#[test]
fn test_parse_provider_case_insensitive() {
    assert_eq!(
        "ANTHROPIC".parse::<ProviderType>().unwrap(),
        ProviderType::Anthropic
    );
    assert_eq!(
        "OpenAI".parse::<ProviderType>().unwrap(),
        ProviderType::Openai
    );
    assert_eq!(
        "OLLAMA".parse::<ProviderType>().unwrap(),
        ProviderType::Ollama
    );
}

#[test]
fn test_unknown_provider_error() {
    let result: Result<ProviderType, ProviderError> = "unknown_provider".parse();
    assert!(result.is_err());

    let error = result.unwrap_err();
    match error {
        ProviderError::UnknownProvider(name) => assert_eq!(name, "unknown_provider"),
        _ => panic!("Expected UnknownProvider error"),
    }
}

#[test]
fn test_provider_type_display() {
    assert_eq!(format!("{}", ProviderType::Anthropic), "anthropic");
    assert_eq!(format!("{}", ProviderType::Openai), "openai");
    assert_eq!(format!("{}", ProviderType::Ollama), "ollama");
}

// Note: Actual provider creation tests require API keys
// Those are manual tests documented in VALIDATION.md
