use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Maximum output characters to prevent context explosion
const MAX_OUTPUT_CHARS: usize = 50_000;

/// Arguments for the Grep tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GrepArgs {
    /// Regex pattern to search for
    pub pattern: String,
    /// Single file path to search (defaults to ".", reads as file)
    pub path: Option<String>,
    /// Case-insensitive search (default: false)
    #[serde(default)]
    pub case_insensitive: bool,
}

/// Error type for Grep tool
#[derive(Debug, thiserror::Error)]
pub enum GrepError {
    #[error("Invalid regex pattern: {0}")]
    RegexError(#[from] regex::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Grep tool for searching file contents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepTool;

impl Tool for GrepTool {
    const NAME: &'static str = "grep";

    type Error = GrepError;
    type Args = GrepArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "grep".to_string(),
            description: r#"
Search for regex patterns in a single file's contents.

Usage notes:
- Searches for the given regex pattern in the specified file
- For Phase 2, only single-file search is supported
- Use case_insensitive=true for case-insensitive matching
- Returns matches in format: path:line_number:line_content
- Output is truncated at 50K characters to prevent context explosion
"#
            .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(GrepArgs))
                .expect("Failed to generate schema for GrepArgs"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Build regex with optional case-insensitive flag
        let regex_pattern = if args.case_insensitive {
            format!("(?i){}", args.pattern)
        } else {
            args.pattern.clone()
        };

        let regex = regex::Regex::new(&regex_pattern)?;

        // Get path (default to current directory)
        let path = args.path.unwrap_or_else(|| ".".to_string());

        // Read file content (async)
        let content = tokio::fs::read_to_string(&path).await?;

        // Search line by line
        let matches: Vec<String> = content
            .lines()
            .enumerate()
            .filter_map(|(i, line)| {
                if regex.is_match(line) {
                    Some(format!("{}:{}:{}", path, i + 1, line))
                } else {
                    None
                }
            })
            .collect();

        // Join matches
        let mut result = matches.join("\n");

        // Truncate if exceeds MAX_OUTPUT_CHARS
        if result.len() > MAX_OUTPUT_CHARS {
            result = format!(
                "{}... (truncated, too many matches)",
                &result[..MAX_OUTPUT_CHARS]
            );
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_grep_tool_finds_matching_lines() {
        let tool = GrepTool;

        // Create a temp file with test content
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Hello World").unwrap();
        writeln!(temp_file, "Hello Rust").unwrap();
        writeln!(temp_file, "Goodbye World").unwrap();

        let args = GrepArgs {
            pattern: "Hello".to_string(),
            path: Some(temp_file.path().to_str().unwrap().to_string()),
            case_insensitive: false,
        };

        let result = tool.call(args).await.unwrap();
        assert_eq!(result.lines().count(), 2);
        assert!(result.contains("Hello World"));
        assert!(result.contains("Hello Rust"));
        assert!(!result.contains("Goodbye"));
    }

    #[tokio::test]
    async fn test_grep_tool_case_insensitive() {
        let tool = GrepTool;

        // Create a temp file with mixed case content
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "HELLO World").unwrap();
        writeln!(temp_file, "hello RUST").unwrap();
        writeln!(temp_file, "HeLLo Code").unwrap();

        let args = GrepArgs {
            pattern: "hello".to_string(),
            path: Some(temp_file.path().to_str().unwrap().to_string()),
            case_insensitive: true,
        };

        let result = tool.call(args).await.unwrap();
        assert_eq!(result.lines().count(), 3);
    }

    #[tokio::test]
    async fn test_grep_tool_format() {
        let tool = GrepTool;

        // Create a temp file
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        writeln!(temp_file, "Line 2").unwrap();

        let args = GrepArgs {
            pattern: "Line".to_string(),
            path: Some(temp_file.path().to_str().unwrap().to_string()),
            case_insensitive: false,
        };

        let result = tool.call(args).await.unwrap();
        // Check format: path:line_number:content
        let lines: Vec<&str> = result.lines().collect();
        assert!(lines[0].contains(":1:"));
        assert!(lines[1].contains(":2:"));
    }

    #[tokio::test]
    async fn test_grep_tool_definition() {
        let tool = GrepTool;
        let definition = tool.definition("test".to_string()).await;
        assert_eq!(definition.name, "grep");
        assert!(definition.description.contains("regex"));
        assert!(definition.description.contains("single-file"));
    }

    #[tokio::test]
    async fn test_grep_tool_truncation() {
        let tool = GrepTool;

        // Create a temp file with lots of matches
        let mut temp_file = NamedTempFile::new().unwrap();
        for i in 0..1000 {
            writeln!(temp_file, "Line {} with pattern", i).unwrap();
        }

        let args = GrepArgs {
            pattern: "pattern".to_string(),
            path: Some(temp_file.path().to_str().unwrap().to_string()),
            case_insensitive: false,
        };

        let result = tool.call(args).await.unwrap();
        // Should be truncated
        assert!(result.contains("truncated"));
        assert!(result.len() <= MAX_OUTPUT_CHARS + 100); // Allow some margin for truncation message
    }
}
