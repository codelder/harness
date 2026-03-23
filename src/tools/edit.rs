use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::Deserialize;
use schemars::JsonSchema;
use tokio::fs;

/// Arguments for the Edit tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct EditArgs {
    /// Absolute path to the file to edit
    pub file_path: String,
    /// Exact string to find and replace
    pub old_string: String,
    /// Replacement string
    pub new_string: String,
}

/// Error type for Edit tool
#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("Failed to read file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Old string not found in file")]
    NotFound,

    #[error("Multiple matches found for old string")]
    MultipleMatches,
}

/// Edit tool for precise string replacement in files
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EditTool;

impl Tool for EditTool {
    const NAME: &'static str = "edit";

    type Error = EditError;
    type Args = EditArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "edit".to_string(),
            description: r#"
Perform precise string replacement in a file.

Usage notes:
- Fails if old_string is not found (returns NotFound error)
- Fails if old_string appears multiple times (returns MultipleMatches error)
- Use this tool for exact, single replacements
- For multiple replacements, call this tool multiple times with unique context
"#.to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(EditArgs))
                .expect("Failed to generate schema for EditArgs"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!("Editing file: {}", args.file_path);

        let content = fs::read_to_string(&args.file_path).await?;

        // Find all matches
        let matches: Vec<_> = content.match_indices(&args.old_string).collect();

        match matches.len() {
            0 => Err(EditError::NotFound),
            1 => {
                let new_content = content.replacen(&args.old_string, &args.new_string, 1);
                fs::write(&args.file_path, &new_content).await?;
                tracing::debug!("Successfully edited {}", args.file_path);
                Ok(format!("Successfully edited {}", args.file_path))
            }
            _ => Err(EditError::MultipleMatches),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_edit_tool_single_match() {
        let tool = EditTool;
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Hello, World!").unwrap();
        writeln!(temp_file, "Line 2").unwrap();
        writeln!(temp_file, "Line 3").unwrap();

        let args = EditArgs {
            file_path: temp_file.path().to_str().unwrap().to_string(),
            old_string: "Line 2".to_string(),
            new_string: "Modified Line".to_string(),
        };

        let result = tool.call(args).await.unwrap();
        assert!(result.contains("Successfully edited"));

        let content = fs::read_to_string(temp_file.path()).await.unwrap();
        assert!(content.contains("Modified Line"));
        assert!(!content.contains("Line 2"));
    }

    #[tokio::test]
    async fn test_edit_tool_not_found() {
        let tool = EditTool;
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Hello, World!").unwrap();

        let args = EditArgs {
            file_path: temp_file.path().to_str().unwrap().to_string(),
            old_string: "NonExistent".to_string(),
            new_string: "Replacement".to_string(),
        };

        let result = tool.call(args).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EditError::NotFound));
    }

    #[tokio::test]
    async fn test_edit_tool_multiple_matches() {
        let tool = EditTool;
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "repeat").unwrap();
        writeln!(temp_file, "other").unwrap();
        writeln!(temp_file, "repeat").unwrap();

        let args = EditArgs {
            file_path: temp_file.path().to_str().unwrap().to_string(),
            old_string: "repeat".to_string(),
            new_string: "replacement".to_string(),
        };

        let result = tool.call(args).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EditError::MultipleMatches));

        // Verify file was NOT modified
        let content = fs::read_to_string(temp_file.path()).await.unwrap();
        assert_eq!(content.matches("repeat").count(), 2);
    }

    #[tokio::test]
    async fn test_edit_tool_definition() {
        let tool = EditTool;
        let definition = tool.definition("test".to_string()).await;
        assert_eq!(definition.name, "edit");
        assert!(definition.description.contains("precise string replacement"));
    }
}
