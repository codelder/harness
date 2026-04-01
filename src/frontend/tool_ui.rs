use crate::frontend::FrontendEventSender;
use std::cell::RefCell;

/// Tool execution context bound to the current async task.
///
/// rig-core invokes tools during a provider request. We use task-local storage
/// to let tools emit UI events that are associated with the current tool call id.
#[derive(Clone)]
pub(crate) struct ToolUiContext {
    pub(crate) turn_id: u64,
    pub(crate) call_id: String,
    pub(crate) event_tx: FrontendEventSender,
}

tokio::task_local! {
    static TOOL_UI_CONTEXT: RefCell<Option<ToolUiContext>>;
}

pub(crate) fn set_tool_ui_context(context: ToolUiContext) {
    let _ = TOOL_UI_CONTEXT.try_with(|cell| {
        *cell.borrow_mut() = Some(context);
    });
}

pub(crate) fn clear_tool_ui_context() {
    let _ = TOOL_UI_CONTEXT.try_with(|cell| {
        *cell.borrow_mut() = None;
    });
}

pub(crate) async fn emit_tool_output(
    delta: String,
    is_stderr: bool,
    emit_fn: impl FnOnce(ToolUiContext, String, bool) -> crate::frontend::FrontendEvent,
) {
    let maybe_context = TOOL_UI_CONTEXT
        .try_with(|cell| cell.borrow().clone())
        .ok()
        .flatten();
    let Some(context) = maybe_context else {
        return;
    };

    // Avoid holding a borrow of `context.event_tx` across the move of `context`
    // into `emit_fn`.
    let event_tx = context.event_tx.clone();
    let event = emit_fn(context, delta, is_stderr);
    let _ = event_tx.emit(event).await;
}
