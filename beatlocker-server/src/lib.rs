#![allow(clippy::derive_partial_eq_without_eq)]
mod api;
mod db;
mod errors;
mod tasks;
mod utils;

pub use api::*;
pub use db::DatabaseOptions;
pub use tasks::*;
pub use utils::*;

use crate::db::Db;
use crate::errors::AppError;
use axum::http::{HeaderMap, HeaderValue, Method};

use axum::{routing::get, Router};
use chrono::{DateTime, Utc};
use const_format::formatcp;
use serde::{Deserialize, Serialize};

use axum::middleware::from_extractor_with_state;
use reqwest_retry::policies::ExponentialBackoff;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::string::ToString;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::level_filters::LevelFilter;

use tracing_subscriber::{EnvFilter, FmtSubscriber};

pub const SERVER_VERSION: &str = git_version::git_version!(fallback = "unknown");
pub const USER_AGENT: &str = formatcp!("beatlocker/{}", SERVER_VERSION);
pub type AppResult<T> = Result<T, AppError>;

#[derive(Clone)]
pub struct ServerOptions {
    pub path: PathBuf,
    pub database: DatabaseOptions,
    pub import_external_metadata: bool,
    pub server_version: String,
    pub discogs_token: Option<String>,
    pub now_provider: Arc<Box<dyn Fn() -> DateTime<Utc> + Send + Sync>>,
    pub subsonic_auth: SubsonicAuth,
}

impl Debug for ServerOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("[ServerOptions]")
    }
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            path: PathBuf::from("."),
            database: DatabaseOptions {
                path: None,
                in_memory: true,
            },
            server_version: "unknown".to_string(),
            import_external_metadata: false,
            discogs_token: None,
            now_provider: Arc::new(Box::new(Utc::now)),
            subsonic_auth: SubsonicAuth::None,
        }
    }
}

pub struct App {
    pub options: ServerOptions,
    pub app: Router,
    pub state: Arc<AppState>,
    pub task_manager: Arc<TaskManager>,
}

#[derive(Clone, Debug)]
pub enum SubsonicAuth {
    None,
    UsernamePassword { username: String, password: String },
}

#[derive(Clone)]
pub struct AppState {
    pub server_version: String,
    pub db: Arc<Db>,
}

impl App {
    pub async fn new(options: ServerOptions) -> AppResult<Self> {
        let state = Arc::new(AppState {
            server_version: options.server_version.clone(),
            db: Arc::new(Db::new(&options.database)?),
        });
        state.db.migrate().await?;

        let task_manager = Arc::new(TaskManager::new(2)?);

        let rest_routes = Router::with_state_arc(state.clone())
            .route("/ping", get(ping))
            .route("/ping.view", get(ping))
            .route("/getAlbumList", get(get_album_list))
            .route("/getAlbumList.view", get(get_album_list))
            .route("/getCoverArt", get(get_cover_art))
            .route("/getCoverArt.view", get(get_cover_art))
            .route("/getIndexes", get(get_indexes))
            .route("/getIndexes.view", get(get_indexes))
            .route("/getLicense", get(get_license))
            .route("/getLicense.view", get(get_license))
            .route("/getMusicDirectory", get(get_music_directory))
            .route("/getMusicDirectory.view", get(get_music_directory))
            .route("/getMusicFolders", get(get_music_folders))
            .route("/getMusicFolders.view", get(get_music_folders))
            .route("/getPlaylist", get(get_playlist))
            .route("/getPlaylist.view", get(get_playlist))
            .route("/getPlaylists", get(get_playlists))
            .route("/getPlaylists.view", get(get_playlists))
            .route("/scrobble", get(ping))
            .route("/scrobble.view", get(ping))
            .route("/search3", get(search3))
            .route("/search3.view", get(search3))
            .route("/stream", get(stream))
            .route("/stream.view", get(stream))
            .route_layer(from_extractor_with_state::<RequireAuth, SubsonicAuth>(
                options.subsonic_auth.clone(),
            ));

        let app = Router::new()
            .nest("/rest", rest_routes)
            .layer(
                CorsLayer::new()
                    .allow_origin("*".parse::<HeaderValue>().unwrap())
                    .allow_methods([Method::GET]),
            )
            .layer(TraceLayer::new_for_http());

        Ok(Self {
            options,
            app,
            state,
            task_manager,
        })
    }

    pub fn task_state(&self) -> Arc<TaskState> {
        Arc::new(TaskState {
            options: self.options.clone(),
            db: self.state.db.clone(),
        })
    }

    pub fn import_all_folders(&self) -> AppResult<TaskMessage> {
        Ok(TaskMessage::ImportFolder {
            state: self.task_state(),
            folder: self.options.path.clone(),
            parent_folder_id: None,
        })
    }

    pub fn import_external_metadata(&self) -> AppResult<TaskMessage> {
        Ok(TaskMessage::ImportExternalMetadata {
            state: self.task_state(),
        })
    }
}

pub fn enable_default_tracing() {
    let filter = EnvFilter::try_from_env("BL_LOG")
        .unwrap_or_else(|_| EnvFilter::new("beatlocker_server=info"))
        .add_directive(LevelFilter::WARN.into())
        .add_directive("reqwest_retry=error".parse().unwrap());

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_ansi(atty::is(atty::Stream::Stdout))
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}
