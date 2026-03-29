use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::fs;

/// Arguments for the Write tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct WriteArgs {
    /// Absolute path to the file to write
    pub file_path: String,
    /// Content to write to the file
    pub content: String,
}

/// Error type for Write tool
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("Failed to write file: {0}")]
    IoError(#[from] std::io::Error),
}

/// Write tool for creating and overwriting files
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WriteTool;

impl Tool for WriteTool {
    const NAME: &'static str = "write";

    type Error = WriteError;
    type Args = WriteArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "write".to_string(),
            description: r#"
Write content to a file, creating or overwriting as needed.

Usage notes:
- Creates parent directories if they don't exist
- Overwrites existing files completely
- Use absolute paths for reliable operation
"#
            .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(WriteArgs))
                .expect("Failed to generate schema for WriteArgs"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!("Writing to file: {}", args.file_path);

        // Create parent directories if needed
        if let Some(parent) = std::path::Path::new(&args.file_path).parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&args.file_path, &args.content).await?;

        tracing::debug!("Successfully wrote to {}", args.file_path);
        Ok(format!("Successfully wrote to {}", args.file_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_write_tool_create_file() {
        let tool = WriteTool;
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = "Hello, World!";

        let args = WriteArgs {
            file_path: file_path.to_str().unwrap().to_string(),
            content: content.to_string(),
        };

        let result = tool.call(args).await.unwrap();
        assert!(result.contains("Successfully wrote"));
        assert!(file_path.exists());

        let read_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(read_content, content);
    }

    #[tokio::test]
    async fn test_write_tool_overwrite() {
        let tool = WriteTool;
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Write initial content
        let args1 = WriteArgs {
            file_path: file_path.to_str().unwrap().to_string(),
            content: "Initial content".to_string(),
        };
        tool.call(args1).await.unwrap();

        // Overwrite with new content
        let args2 = WriteArgs {
            file_path: file_path.to_str().unwrap().to_string(),
            content: "New content".to_string(),
        };
        tool.call(args2).await.unwrap();

        let read_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(read_content, "New content");
        assert_ne!(read_content, "Initial content");
    }

    #[tokio::test]
    async fn test_write_tool_create_parent_dirs() {
        let tool = WriteTool;
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nested/dir/test.txt");

        let args = WriteArgs {
            file_path: file_path.to_str().unwrap().to_string(),
            content: "Content in nested dir".to_string(),
        };

        let result = tool.call(args).await.unwrap();
        assert!(result.contains("Successfully wrote"));
        assert!(file_path.exists());
        assert!(file_path.parent().unwrap().exists());
    }

    #[tokio::test]
    async fn test_write_tool_definition() {
        let tool = WriteTool;
        let definition = tool.definition("test".to_string()).await;
        assert_eq!(definition.name, "write");
        assert!(definition.description.contains("Write content"));
    }
}
