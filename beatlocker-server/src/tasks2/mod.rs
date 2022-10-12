mod extract_metadata;
mod import_folder_task;
mod providers;

use crate::tasks2::import_folder_task::import_folder;
use crate::{AppResult, Db};
use chrono::{DateTime, Utc};
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use thread_priority::{set_current_thread_priority, ThreadPriority};
use tokio::sync::{mpsc, oneshot, Barrier};
use tokio::task::JoinSet;
use tokio::{runtime, task};
use tracing::{debug, error, info, trace};
use uuid::Uuid;

pub struct TaskManager {
    #[allow(dead_code)]
    thread: JoinHandle<()>,
    message_tx: mpsc::Sender<TaskEnvelope>,
    shutdown_tx: mpsc::Sender<()>,
    shutdown_barrier: Arc<Barrier>,
}

struct TaskEnvelope {
    message: TaskMessage,
    reply_tx: oneshot::Sender<TaskReply>,
}

impl Debug for TaskEnvelope {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("[TaskEnvelope]")
    }
}

#[derive(Debug)]
pub enum TaskMessage {
    Ping,
    ImportFolder {
        state: Arc<TaskState>,
        folder: PathBuf,
        parent_folder_id: Option<Uuid>,
    },
}

#[derive(Debug, PartialEq)]
pub enum TaskReply {
    Pong,
    ImportFolder(PathBuf),
}

pub struct TaskState {
    pub db: Arc<Db>,
    pub now_provider: Arc<Box<dyn Fn() -> DateTime<Utc> + Send + Sync>>,
    //provider_list: Arc<InfoProviderList>,
    pub root_path: PathBuf,
}

impl Debug for TaskState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("[TaskState]")
    }
}

impl TaskManager {
    pub fn new(num_threads: usize) -> AppResult<Self> {
        let (message_tx, mut message_rx) = mpsc::channel::<TaskEnvelope>(32);
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        let shutdown_barrier = Arc::new(Barrier::new(3));

        let barrier = shutdown_barrier.clone();
        let thread_barrier = shutdown_barrier.clone();
        let thread = thread::spawn(move || {
            let runtime = runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("tasks")
                .worker_threads(num_threads)
                .on_thread_start(move || {
                    debug!("Started worker thread");
                    set_current_thread_priority(ThreadPriority::Min)
                        .expect("Could not set task thread priority")
                })
                .on_thread_stop(move || debug!("Stopped worker thread"))
                .build()
                .expect("Could not spawn task manager runtime");

            runtime.spawn(async move {
                let mut running_tasks = vec![];
                loop {
                    tokio::select! {
                        Some(envelope) = message_rx.recv() => {
                            let message = envelope.message;
                            trace!(?message);
                            match message {
                                TaskMessage::Ping => {
                                    let _ = envelope.reply_tx.send(TaskReply::Pong);
                                },
                                TaskMessage::ImportFolder { state, folder, parent_folder_id } => {
                                    running_tasks.push(task::spawn(async move {
                                        import_folder(state, folder.as_path(), parent_folder_id).await.unwrap_or_else(|e| {
                                            error!(?e, "Error in background task");
                                        });
                                        let _ = envelope.reply_tx.send(TaskReply::ImportFolder(folder));
                                    }));
                                }
                            }
                        },
                        Some(_) = shutdown_rx.recv() => {
                            info!("Shutting down background task manager");
                            barrier.wait().await;
                            break;
                        },
                        else => {
                            // Got shutdown without a message
                            barrier.wait().await;
                            break;
                        }
                    }
                }
            });

            runtime.block_on(async {
                thread_barrier.wait().await;
            });
        });

        Ok(Self {
            thread,
            message_tx,
            shutdown_tx,
            shutdown_barrier,
        })
    }

    pub async fn send(&self, message: TaskMessage) -> AppResult<TaskReply> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.message_tx
            .send(TaskEnvelope { message, reply_tx })
            .await?;
        let reply = reply_rx.await?;
        Ok(reply)
    }

    pub async fn shutdown(&self) -> AppResult<()> {
        self.shutdown_tx.send(()).await?;
        self.shutdown_barrier.wait().await;
        Ok(())
    }
}

async fn await_join_set(mut set: JoinSet<AppResult<()>>) -> AppResult<()> {
    while let Some(result) = set.join_next().await {
        if let Err(e) = result? {
            error!(?e, "Error in background task");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppResult;

    #[test]
    fn can_spawn_task_and_shutdown() -> AppResult<()> {
        let mgr = TaskManager::new(1)?;

        let rt = runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(async {
            let reply = mgr.send(TaskMessage::Ping).await.unwrap();
            assert_eq!(reply, TaskReply::Pong);
            mgr.shutdown().await.unwrap();
        });

        Ok(())
    }
}
