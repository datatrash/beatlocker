mod extract_metadata;
mod import_external_metadata_task;
mod import_folder_task;
mod optimize_database_task;
mod removed_deleted_files_task;

use crate::db::DbCoverArt;
use crate::tasks::import_external_metadata_task::import_external_metadata;
use crate::tasks::import_folder_task::import_folder;
use crate::tasks::optimize_database_task::optimize_database;
use crate::tasks::removed_deleted_files_task::remove_deleted_files;
use crate::{reqwest_client, str_to_uuid, AppResult, Db, ServerOptions};
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use tokio::sync::{mpsc, oneshot, Barrier};
use tokio::task::JoinSet;
use tokio::{runtime, task};
use tracing::{debug, error, info};
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

#[derive(Clone, Debug)]
pub enum TaskMessage {
    Ping,
    ImportFolder {
        state: Arc<TaskState>,
        folder: PathBuf,
        parent_folder_id: Option<Uuid>,
    },
    ImportExternalMetadata {
        state: Arc<TaskState>,
    },
    OptimizeDatabase {
        state: Arc<TaskState>,
    },
    RemoveDeletedFiles {
        state: Arc<TaskState>,
    },
}

#[derive(Debug, PartialEq)]
pub enum TaskReply {
    Pong,
    ImportFolder(PathBuf),
    ImportExternalMetadata,
    OptimizeDatabase,
    RemoveDeletedFiles,
}

pub struct TaskState {
    pub options: ServerOptions,
    pub db: Arc<Db>,
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
        let shutdown_barrier = Arc::new(Barrier::new(2));

        let thread_barrier = shutdown_barrier.clone();
        let thread = thread::spawn(move || {
            let runtime = runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("tasks")
                .worker_threads(num_threads)
                .on_thread_start(move || {
                    debug!("Started worker thread");
                })
                .on_thread_stop(move || debug!("Stopped worker thread"))
                .build()
                .expect("Could not spawn task manager runtime");

            runtime.spawn(async move {
                loop {
                    tokio::select! {
                        Some(envelope) = message_rx.recv() => {
                            let message = envelope.message;
                            match message {
                                TaskMessage::Ping => {
                                    envelope.reply_tx.send(TaskReply::Pong).unwrap();
                                },
                                TaskMessage::ImportFolder { state, folder, parent_folder_id } => {
                                    task::spawn(async move {
                                        import_folder(state, folder.as_path(), parent_folder_id).await.unwrap_or_else(|e| {
                                            error!(?e, "Error when importing folders");
                                        });
                                        let _ = envelope.reply_tx.send(TaskReply::ImportFolder(folder));
                                    });
                                }
                                TaskMessage::ImportExternalMetadata { state } => {
                                    task::spawn(async move {
                                        import_external_metadata(state).await.unwrap_or_else(|e| {
                                            error!(?e, "Error when importing Discogs metadata");
                                        });
                                        let _ = envelope.reply_tx.send(TaskReply::ImportExternalMetadata);
                                    });
                                }
                                TaskMessage::OptimizeDatabase { state } => {
                                    task::spawn(async move {
                                        optimize_database(state).await.unwrap_or_else(|e| {
                                            error!(?e, "Error when optimizing database");
                                        });
                                        let _ = envelope.reply_tx.send(TaskReply::OptimizeDatabase);
                                    });
                                }
                                TaskMessage::RemoveDeletedFiles { state } => {
                                    task::spawn(async move {
                                        remove_deleted_files(state).await.unwrap_or_else(|e| {
                                            error!(?e, "Error when removing deleted files");
                                        });
                                        let _ = envelope.reply_tx.send(TaskReply::RemoveDeletedFiles);
                                    });
                                }
                            }
                        },
                        Some(_) = shutdown_rx.recv() => {
                            info!("Shutting down background task manager");
                            break;
                        },
                        else => {
                            // Got shutdown without a message
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

async fn insert_cover_art(db: &Db, url: &str) -> AppResult<Uuid> {
    let client = reqwest_client();

    // Find out the actual (potentially redirected) url first
    let head = client.head(url).send().await?;
    let url = head.url().to_string();

    // only cover the path in the UUID, since hostnames may differ sometimes due to CDNs etc
    let cover_art_id = str_to_uuid(head.url().path());

    match db.find_cover_art(cover_art_id).await? {
        Some(id) => Ok(id),
        None => {
            let response = client.get(&url).send().await?;
            let data = response.bytes().await?.to_vec();
            Ok(db
                .insert_cover_art_if_not_exists(&DbCoverArt { cover_art_id, data })
                .await?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppResult;

    #[tokio::test]
    async fn can_spawn_task_and_shutdown() -> AppResult<()> {
        let mgr = TaskManager::new(4)?;
        let reply = mgr.send(TaskMessage::Ping).await.unwrap();
        assert_eq!(reply, TaskReply::Pong);
        mgr.shutdown().await.unwrap();
        Ok(())
    }
}
