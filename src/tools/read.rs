use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::Deserialize;
use schemars::JsonSchema;
use tokio::fs;

/// Maximum output characters to prevent context explosion
const MAX_OUTPUT_CHARS: usize = 50_000;

/// Arguments for the Read tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ReadArgs {
    /// Absolute path to the file to read
    pub file_path: String,
    /// Optional line number to start reading from
    pub offset: Option<usize>,
    /// Optional number of lines to read
    pub limit: Option<usize>,
}

/// Error type for Read tool
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("Failed to read file: {0}")]
    IoError(#[from] std::io::Error),
}

/// Read tool for reading file contents
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReadTool;

impl Tool for ReadTool {
    const NAME: &'static str = "read";

    type Error = ReadError;
    type Args = ReadArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "read".to_string(),
            description: r#"
Read file contents from the filesystem.

Usage notes:
- Use this tool to read file contents
- Supports offset and limit for reading partial files
- By default, reads up to 2000 lines starting from the beginning
- Large files are truncated to prevent context explosion
"#.to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(ReadArgs))
                .expect("Failed to generate schema for ReadArgs"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!("Reading file: {}", args.file_path);

        let content = fs::read_to_string(&args.file_path).await?;

        // Apply offset and limit on lines
        let lines: Vec<&str> = content.lines().collect();
        let offset = args.offset.unwrap_or(0);
        let limit = args.limit.unwrap_or(lines.len());

        let selected: Vec<&str> = lines
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();

        let mut result = selected.join("\n");

        // Truncate if too large
        if result.len() > MAX_OUTPUT_CHARS {
            result = format!("{}... (truncated, file too large)", &result[..MAX_OUTPUT_CHARS]);
        }

        tracing::debug!("Read {} characters from file", result.len());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_read_tool_valid_file() {
        let tool = ReadTool;
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Hello, World!").unwrap();
        writeln!(temp_file, "Line 2").unwrap();
        writeln!(temp_file, "Line 3").unwrap();

        let args = ReadArgs {
            file_path: temp_file.path().to_str().unwrap().to_string(),
            offset: None,
            limit: None,
        };

        let result = tool.call(args).await.unwrap();
        assert!(result.contains("Hello, World!"));
        assert!(result.contains("Line 2"));
        assert!(result.contains("Line 3"));
    }

    #[tokio::test]
    async fn test_read_tool_nonexistent_file() {
        let tool = ReadTool;
        let args = ReadArgs {
            file_path: "/nonexistent/file.txt".to_string(),
            offset: None,
            limit: None,
        };

        let result = tool.call(args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_tool_offset_limit() {
        let tool = ReadTool;
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        writeln!(temp_file, "Line 2").unwrap();
        writeln!(temp_file, "Line 3").unwrap();
        writeln!(temp_file, "Line 4").unwrap();
        writeln!(temp_file, "Line 5").unwrap();

        let args = ReadArgs {
            file_path: temp_file.path().to_str().unwrap().to_string(),
            offset: Some(1),
            limit: Some(2),
        };

        let result = tool.call(args).await.unwrap();
        assert!(!result.contains("Line 1"));
        assert!(result.contains("Line 2"));
        assert!(result.contains("Line 3"));
        assert!(!result.contains("Line 4"));
        assert!(!result.contains("Line 5"));
    }

    #[tokio::test]
    async fn test_read_tool_definition() {
        let tool = ReadTool;
        let definition = tool.definition("test".to_string()).await;
        assert_eq!(definition.name, "read");
        assert!(definition.description.contains("Read file"));
    }

    #[tokio::test]
    async fn test_read_tool_truncation() {
        let tool = ReadTool;
        let mut temp_file = NamedTempFile::new().unwrap();

        // Create content larger than MAX_OUTPUT_CHARS
        let large_content = "x".repeat(MAX_OUTPUT_CHARS + 1000);
        temp_file.write_all(large_content.as_bytes()).unwrap();

        let args = ReadArgs {
            file_path: temp_file.path().to_str().unwrap().to_string(),
            offset: None,
            limit: None,
        };

        let result = tool.call(args).await.unwrap();
        assert!(result.len() < large_content.len());
        assert!(result.contains("truncated"));
    }
}
