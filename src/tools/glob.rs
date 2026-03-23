use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Arguments for the Glob tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GlobArgs {
    /// Glob pattern to match (e.g., "*.rs" or "**/*.ts")
    pub pattern: String,
    /// Base directory to search (defaults to ".")
    pub path: Option<String>,
}

/// Error type for Glob tool
#[derive(Debug, thiserror::Error)]
pub enum GlobError {
    #[error("Glob pattern error: {0}")]
    PatternError(#[from] glob::PatternError),

    #[error("Glob error: {0}")]
    GlobError(#[from] glob::GlobError),
}

/// Glob tool for finding files matching patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobTool;

impl Tool for GlobTool {
    const NAME: &'static str = "glob";

    type Error = GlobError;
    type Args = GlobArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "glob".to_string(),
            description: r#"
Find files matching a glob pattern.

Usage notes:
- Supports standard glob patterns like *.rs, *.toml
- Supports ** for recursive directory matching
- Use the path parameter to specify a base directory (defaults to current directory)
- Returns newline-separated list of matching file paths
"#.to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(GlobArgs))
                .expect("Failed to generate schema for GlobArgs"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let base_path = args.path.unwrap_or_else(|| ".".to_string());
        let full_pattern = format!("{}/{}", base_path, args.pattern);

        let paths: Vec<String> = glob::glob(&full_pattern)?
            .filter_map(|entry| entry.ok())
            .map(|path| path.display().to_string())
            .collect();

        Ok(paths.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_glob_tool_finds_rs_files() {
        let tool = GlobTool;
        let args = GlobArgs {
            pattern: "*.rs".to_string(),
            path: Some("src/tools".to_string()),
        };
        let result = tool.call(args).await.unwrap();
        assert!(result.contains("glob.rs"));
    }

    #[tokio::test]
    async fn test_glob_tool_respects_path_parameter() {
        let tool = GlobTool;
        let args = GlobArgs {
            pattern: "*.rs".to_string(),
            path: Some("src/tools".to_string()),
        };
        let result = tool.call(args).await.unwrap();
        // All results should start with src/tools
        for line in result.lines() {
            assert!(line.starts_with("src/tools"), "Path {} doesn't start with src/tools", line);
        }
    }

    #[tokio::test]
    async fn test_glob_tool_recursive_pattern() {
        let tool = GlobTool;
        let args = GlobArgs {
            pattern: "**/*.rs".to_string(),
            path: Some("src".to_string()),
        };
        let result = tool.call(args).await.unwrap();
        // Should find files in nested directories
        assert!(result.lines().count() > 0);
        // Should contain files from subdirectories
        let has_nested = result.lines().any(|line| line.contains("/tools/"));
        assert!(has_nested, "Should find files in nested directories");
    }

    #[tokio::test]
    async fn test_glob_tool_definition() {
        let tool = GlobTool;
        let definition = tool.definition("test".to_string()).await;
        assert_eq!(definition.name, "glob");
        assert!(definition.description.contains("glob pattern"));
        assert!(definition.description.contains("**"));
    }
}
