use std::str::FromStr;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Maximum number of todo items allowed
const MAX_ITEMS: usize = 20;

/// Task status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

/// Parse TodoStatus from string (case-insensitive)
impl FromStr for TodoStatus {
    type Err = TodoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(TodoStatus::Pending),
            "in_progress" | "in-progress" => Ok(TodoStatus::InProgress),
            "completed" => Ok(TodoStatus::Completed),
            _ => Err(TodoError::InvalidStatus(s.to_string())),
        }
    }
}

/// A single todo item
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoItem {
    pub id: u32,
    pub text: String,
    pub status: TodoStatus,
}

impl TodoItem {
    /// Create a new todo item
    pub fn new(id: u32, text: String, status: TodoStatus) -> Self {
        Self { id, text, status }
    }
}

/// Error type for TodoManager operations
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TodoError {
    #[error("Too many todos: {0} (max 20)")]
    TooManyItems(usize),
    #[error("Multiple tasks in progress: {0} (only one allowed)")]
    MultipleInProgress(usize),
    #[error("Invalid status '{0}' (expected: pending, in_progress, completed)")]
    InvalidStatus(String),
    #[error("Item #{0}: text is required")]
    MissingText(u32),
    #[error("Invalid item ID: {0} (must be 1-20)")]
    InvalidId(u32),
    #[error("Duplicate item ID: {0}")]
    DuplicateId(u32),
}

/// Manager for todo items with constraint validation
#[derive(Debug, Clone, Default)]
pub struct TodoManager {
    items: Vec<TodoItem>,
}

impl TodoManager {
    /// Create a new empty TodoManager
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Check if the manager has no items
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Update the todo list with new items
    /// Validates constraints:
    /// - Max 20 items
    /// - Only one in_progress at a time
    pub fn update(&mut self, items: Vec<TodoItem>) -> Result<String, TodoError> {
        // Validate max 20 items
        if items.len() > MAX_ITEMS {
            return Err(TodoError::TooManyItems(items.len()));
        }

        // Validate text is not empty
        for item in &items {
            if item.text.trim().is_empty() {
                return Err(TodoError::MissingText(item.id));
            }
        }

        // Validate only one in_progress
        let in_progress_count = items
            .iter()
            .filter(|i| i.status == TodoStatus::InProgress)
            .count();
        if in_progress_count > 1 {
            return Err(TodoError::MultipleInProgress(in_progress_count));
        }

        self.items = items;
        Ok(self.render())
    }

    /// Render todos as formatted string
    /// Format:
    /// [ ] #1: Task description
    /// [>] #2: In progress task
    /// [x] #3: Completed task
    ///
    /// (done/total)
    pub fn render(&self) -> String {
        if self.items.is_empty() {
            return "No todos.".to_string();
        }

        let mut lines: Vec<String> = self
            .items
            .iter()
            .map(|item| {
                let marker = match item.status {
                    TodoStatus::Pending => "[ ]",
                    TodoStatus::InProgress => "[>]",
                    TodoStatus::Completed => "[x]",
                };
                format!("{} #{}: {}", marker, item.id, item.text)
            })
            .collect();

        let done = self
            .items
            .iter()
            .filter(|i| i.status == TodoStatus::Completed)
            .count();
        lines.push(format!("\n({}/{})", done, self.items.len()));

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_manager_new() {
        let manager = TodoManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.render(), "No todos.");
    }

    #[test]
    fn test_todo_manager_update_valid_items() {
        let mut manager = TodoManager::new();
        let items = vec![
            TodoItem::new(1, "First task".to_string(), TodoStatus::Pending),
            TodoItem::new(2, "Second task".to_string(), TodoStatus::InProgress),
        ];

        let result = manager.update(items).unwrap();
        assert!(result.contains("[ ] #1: First task"));
        assert!(result.contains("[>] #2: Second task"));
        assert!(result.contains("(0/2)"));
    }

    #[test]
    fn test_todo_manager_update_too_many_items() {
        let mut manager = TodoManager::new();
        let items: Vec<TodoItem> = (1..=21)
            .map(|i| TodoItem::new(i, format!("Task {}", i), TodoStatus::Pending))
            .collect();

        let result = manager.update(items);
        assert!(matches!(result, Err(TodoError::TooManyItems(21))));
    }

    #[test]
    fn test_todo_manager_update_multiple_in_progress() {
        let mut manager = TodoManager::new();
        let items = vec![
            TodoItem::new(1, "Task 1".to_string(), TodoStatus::InProgress),
            TodoItem::new(2, "Task 2".to_string(), TodoStatus::InProgress),
            TodoItem::new(3, "Task 3".to_string(), TodoStatus::Pending),
        ];

        let result = manager.update(items);
        assert!(matches!(result, Err(TodoError::MultipleInProgress(2))));
    }

    #[test]
    fn test_todo_manager_render_markers() {
        let mut manager = TodoManager::new();
        let items = vec![
            TodoItem::new(1, "Pending task".to_string(), TodoStatus::Pending),
            TodoItem::new(2, "In progress task".to_string(), TodoStatus::InProgress),
            TodoItem::new(3, "Completed task".to_string(), TodoStatus::Completed),
        ];

        let result = manager.update(items).unwrap();
        assert!(result.contains("[ ] #1: Pending task"));
        assert!(result.contains("[>] #2: In progress task"));
        assert!(result.contains("[x] #3: Completed task"));
    }

    #[test]
    fn test_todo_manager_render_progress_count() {
        let mut manager = TodoManager::new();
        let items = vec![
            TodoItem::new(1, "Task 1".to_string(), TodoStatus::Completed),
            TodoItem::new(2, "Task 2".to_string(), TodoStatus::Completed),
            TodoItem::new(3, "Task 3".to_string(), TodoStatus::Pending),
        ];

        let result = manager.update(items).unwrap();
        assert!(result.contains("(2/3)"));
    }

    #[test]
    fn test_todo_status_from_str_valid() {
        assert_eq!("pending".parse::<TodoStatus>(), Ok(TodoStatus::Pending));
        assert_eq!("PENDING".parse::<TodoStatus>(), Ok(TodoStatus::Pending));
        assert_eq!("in_progress".parse::<TodoStatus>(), Ok(TodoStatus::InProgress));
        assert_eq!("IN_PROGRESS".parse::<TodoStatus>(), Ok(TodoStatus::InProgress));
        assert_eq!("in-progress".parse::<TodoStatus>(), Ok(TodoStatus::InProgress));
        assert_eq!("completed".parse::<TodoStatus>(), Ok(TodoStatus::Completed));
        assert_eq!("COMPLETED".parse::<TodoStatus>(), Ok(TodoStatus::Completed));
    }

    #[test]
    fn test_todo_status_from_str_invalid() {
        let result = "invalid".parse::<TodoStatus>();
        assert!(matches!(result, Err(TodoError::InvalidStatus(_))));

        let result = "done".parse::<TodoStatus>();
        assert!(matches!(result, Err(TodoError::InvalidStatus(_))));
    }

    #[test]
    fn test_todo_manager_empty_text() {
        let mut manager = TodoManager::new();
        let items = vec![
            TodoItem::new(1, "".to_string(), TodoStatus::Pending),
        ];

        let result = manager.update(items);
        assert!(matches!(result, Err(TodoError::MissingText(1))));
    }
}
