use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::Deserialize;
use schemars::JsonSchema;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::planning::{TodoManager, TodoItem, TodoStatus, TodoError};

/// Arguments for the Todo tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TodoArgs {
    /// List of todo items (replaces entire list)
    pub items: Vec<TodoItemInput>,
}

/// Input structure for a single todo item
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TodoItemInput {
    /// Task ID (1-20)
    pub id: u32,
    /// Task description
    pub text: String,
    /// Task status: pending, in_progress, completed
    pub status: TodoStatus,
}

/// Todo tool for task management
#[derive(Debug, Clone)]
pub struct TodoTool {
    manager: Arc<Mutex<TodoManager>>,
}

impl TodoTool {
    /// Create a new TodoTool with a shared TodoManager reference
    pub fn new(manager: Arc<Mutex<TodoManager>>) -> Self {
        Self { manager }
    }

    /// Create a new TodoTool with a fresh TodoManager
    pub fn with_fresh_manager() -> Self {
        Self {
            manager: Arc::new(Mutex::new(TodoManager::new())),
        }
    }

    /// Get a clone of the shared TodoManager reference
    /// Used by Session to reset rounds_since_todo counter
    pub fn get_manager(&self) -> Arc<Mutex<TodoManager>> {
        self.manager.clone()
    }
}

impl Tool for TodoTool {
    const NAME: &'static str = "todo";

    type Error = TodoError;
    type Args = TodoArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "todo".to_string(),
            description: r#"
Update task list. Track progress on multi-step tasks.

Usage notes:
- Use this tool to track your progress on multi-step tasks
- Each call replaces the entire todo list
- Maximum 20 todos allowed
- Only one task can be in_progress at a time
- Status options: pending, in_progress, completed
"#.to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(TodoArgs))
                .expect("Failed to generate schema for TodoArgs"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!("Todo tool called with {} items", args.items.len());

        // Convert TodoItemInput to TodoItem with validation
        let mut items = Vec::with_capacity(args.items.len());
        for input in args.items {
            let text = input.text.trim();
            if text.is_empty() {
                return Err(TodoError::MissingText(input.id));
            }
            items.push(TodoItem {
                id: input.id,
                text: text.to_string(),
                status: input.status,
            });
        }

        // Update the shared TodoManager
        let mut manager = self.manager.lock().await;
        manager.update(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_todo_tool_definition_name() {
        let tool = TodoTool::with_fresh_manager();
        let definition = tool.definition("test".to_string()).await;
        assert_eq!(definition.name, "todo");
    }

    #[tokio::test]
    async fn test_todo_tool_definition_schema_has_items() {
        let tool = TodoTool::with_fresh_manager();
        let definition = tool.definition("test".to_string()).await;
        let params = definition.parameters.as_object().unwrap();
        let properties = params.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("items"));
    }

    #[tokio::test]
    async fn test_todo_tool_call_valid_items() {
        let tool = TodoTool::with_fresh_manager();
        let args = TodoArgs {
            items: vec![
                TodoItemInput {
                    id: 1,
                    text: "First task".to_string(),
                    status: TodoStatus::Pending,
                },
                TodoItemInput {
                    id: 2,
                    text: "Second task".to_string(),
                    status: TodoStatus::InProgress,
                },
            ],
        };

        let result = tool.call(args).await.unwrap();
        assert!(result.contains("[ ] #1: First task"));
        assert!(result.contains("[>] #2: Second task"));
    }

    #[tokio::test]
    async fn test_todo_tool_call_too_many_items() {
        let tool = TodoTool::with_fresh_manager();
        let items: Vec<TodoItemInput> = (1..=21)
            .map(|id| TodoItemInput {
                id,
                text: format!("Task {}", id),
                status: TodoStatus::Pending,
            })
            .collect();

        let args = TodoArgs { items };
        let result = tool.call(args).await;
        assert!(matches!(result, Err(TodoError::TooManyItems(21))));
    }

    #[tokio::test]
    async fn test_todo_tool_call_multiple_in_progress() {
        let tool = TodoTool::with_fresh_manager();
        let args = TodoArgs {
            items: vec![
                TodoItemInput {
                    id: 1,
                    text: "Task 1".to_string(),
                    status: TodoStatus::InProgress,
                },
                TodoItemInput {
                    id: 2,
                    text: "Task 2".to_string(),
                    status: TodoStatus::InProgress,
                },
            ],
        };

        let result = tool.call(args).await;
        assert!(matches!(result, Err(TodoError::MultipleInProgress(2))));
    }

    #[tokio::test]
    async fn test_todo_tool_call_completed_status() {
        let tool = TodoTool::with_fresh_manager();
        let args = TodoArgs {
            items: vec![TodoItemInput {
                id: 1,
                text: "Task 1".to_string(),
                status: TodoStatus::Completed,
            }],
        };

        let result = tool.call(args).await.unwrap();
        assert!(result.contains("[x] #1: Task 1"));
    }

    #[tokio::test]
    async fn test_todo_tool_new_with_manager() {
        let manager = Arc::new(Mutex::new(TodoManager::new()));
        let tool = TodoTool::new(manager.clone());

        // Verify the tool shares the same manager
        assert!(Arc::ptr_eq(&tool.manager, &manager));
    }

    #[tokio::test]
    async fn test_todo_tool_get_manager() {
        let tool = TodoTool::with_fresh_manager();
        let manager = tool.get_manager();

        // Verify we can use the manager
        let mgr = manager.lock().await;
        assert!(mgr.is_empty());
    }
}
