use std::sync::Arc;

use agent_memory::MemoryQuery;
use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

use crate::app::App;
use crate::components::trace::MemoryRow;
use crate::components::Command;

use super::push_status_error;

pub(crate) async fn dispatch(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    app: &mut App,
    command: Command,
) {
    match command {
        Command::LoadMemories {
            scope,
            keywords,
            limit,
        } => {
            match runtime.memory_store() {
                Some(memory_store) => {
                    match memory_store
                        .query(MemoryQuery {
                            scope,
                            keywords,
                            limit,
                            session_id: None,
                            workspace_id: None,
                        })
                        .await
                    {
                        Ok(entries) => {
                            app.trace.set_memory_rows(
                                entries.into_iter().map(MemoryRow::from).collect(),
                            );
                        }
                        Err(e) => {
                            push_status_error(app, format!("[memory query error: {e}]"));
                        }
                    }
                }
                None => {
                    app.trace.set_memory_rows(Vec::new());
                }
            }
            app.state.render_scheduler.mark_dirty();
        }

        Command::DeleteMemory { memory_id } => {
            match runtime.memory_store() {
                Some(memory_store) => {
                    if let Err(e) = memory_store.delete(&memory_id).await {
                        push_status_error(app, format!("[memory delete error: {e}]"));
                    } else {
                        app.trace.remove_memory_row(&memory_id);
                    }
                }
                None => {
                    app.trace.remove_memory_row(&memory_id);
                }
            }
            app.state.render_scheduler.mark_dirty();
        }

        _ => {}
    }
}
