use harness::{agent_loop, with_retry, Message, Role, AgentError};

#[test]
fn test_message_types() {
    let system = Message::system("System prompt");
    assert_eq!(system.role, Role::System);

    let user = Message::user("User message");
    assert_eq!(user.role, Role::User);

    let assistant = Message::assistant("Assistant response");
    assert_eq!(assistant.role, Role::Assistant);
}

#[test]
fn test_message_cloning() {
    let original = Message::user("Test message");
    let cloned = original.clone();
    assert_eq!(original.content, cloned.content);
    assert_eq!(original.role, cloned.role);
}

#[tokio::test]
async fn test_with_retry_immediate_success() {
    let result = with_retry(3, || async { Ok::<_, AgentError>(42) }).await;
    assert_eq!(result.unwrap(), 42);
}

#[tokio::test]
async fn test_with_retry_non_retryable_error() {
    let result: Result<i32, AgentError> = with_retry(3, || async {
        Err(AgentError::Auth("invalid".to_string()))
    })
    .await;

    assert!(result.is_err());
    matches!(result.unwrap_err(), AgentError::Auth(_));
}

// Note: Actual agent_loop tests require a mock provider
// Those tests will be added in the CLI integration phase
// For now, we verify that the types are accessible

#[test]
fn test_agent_loop_types_exist() {
    // This is a compile-time check - if this compiles, the types exist
    let _msg = Message::user("test");
    let _role = Role::User;

    // Verify agent_loop is in scope (compile-time check)
    // The function signature is: async fn agent_loop(&mut Vec<Message>, &LlmProvider) -> Result<String, AgentError>
    // We can't call it without a real provider, but we can verify it's exported
    use harness::agent_loop;
    let _ = agent_loop;
}
