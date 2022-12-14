use beatlocker_server::{
    enable_default_tracing, App, AppResult, DatabaseOptions, ServerOptions, SubsonicAuth,
    SERVER_VERSION,
};
use clap::Parser;
use futures::FutureExt;
use governor::{Jitter, Quota, RateLimiter};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::time::Duration;
use tokio::signal;
use tokio::time::sleep;
use tracing::{info, warn};

#[derive(Parser)]
#[clap(
    name = "Beatlocker",
    author = "datatrash",
    version = SERVER_VERSION,
)]
struct Cli {
    /// Path to audio library
    #[arg(long, env = "BL_LIBRARY_PATH")]
    library_path: String,

    /// Path to a data folder Beatlocker may use
    #[arg(long, default_value = ".", env = "BL_DATA_PATH")]
    data_path: String,

    /// Discogs API token
    #[arg(long, env = "BL_DISCOGS_TOKEN")]
    discogs_token: Option<String>,

    /// last.fm API key
    #[arg(long, env = "BL_LASTFM_API_KEY")]
    lastfm_api_key: Option<String>,

    /// Run fully in-memory (no SQLite database will be created)
    #[arg(long)]
    run_in_memory: bool,

    /// Username to use for authentication
    #[arg(long, requires = "auth_password", env = "BL_AUTH_USER")]
    auth_user: Option<String>,

    /// Password to use for authentication
    #[arg(long, requires = "auth_user", env = "BL_AUTH_PASSWORD")]
    auth_password: Option<String>,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let cli = Cli::parse();

    enable_default_tracing();

    info!("beatlocker {}", SERVER_VERSION);
    info!("Server starting...");

    let subsonic_auth = match (cli.auth_user, cli.auth_password) {
        (Some(username), Some(password)) => SubsonicAuth::UsernamePassword { username, password },
        _ => SubsonicAuth::None,
    };

    let options = ServerOptions {
        path: PathBuf::from(cli.library_path),
        database: DatabaseOptions {
            path: Some(PathBuf::from(cli.data_path)),
            in_memory: cli.run_in_memory,
        },
        server_version: SERVER_VERSION.to_string(),
        import_external_metadata: true,
        discogs_token: cli.discogs_token,
        lastfm_api_key: cli.lastfm_api_key,
        subsonic_auth,
        ..Default::default()
    };

    if options.discogs_token.is_none() {
        info!("No Discogs API token was found. Discogs will not be queried.");
    }
    if let SubsonicAuth::None = &options.subsonic_auth {
        warn!("No authorization has been set up. Make sure this server isn't public.");
    }

    let app = App::new(options).await?;
    let shutdown_signal = shutdown_signal().shared();
    let server = axum::Server::bind(&"0.0.0.0:2222".parse().unwrap())
        .serve(app.app.clone().into_make_service())
        .with_graceful_shutdown(shutdown_signal.clone());

    info!("Server started");

    let mgr = app.task_manager.clone();
    let tasks = vec![
        app.import_all_folders().await?,
        app.import_external_metadata().await?,
        app.remove_deleted_files().await?,
        app.optimize_database().await?,
    ];
    let join = tokio::spawn(async move {
        let lim = RateLimiter::direct(Quota::per_hour(NonZeroU32::new(1u32).unwrap()));
        let jitter = Jitter::new(Duration::from_secs(60 * 15), Duration::from_secs(60 * 3));

        loop {
            lim.until_ready_with_jitter(jitter).await;
            for task in &tasks {
                let _ = mgr.send(task.clone()).await;
            }
        }
    });

    let delayed_shutdown = shutdown_signal.then(|_| async move {
        sleep(Duration::from_secs(5)).await;
    });
    tokio::select! {
        _ = server => {},
        _ = delayed_shutdown => {}
    }

    app.task_manager.shutdown().await?;
    join.abort();

    info!("Server is shutdown");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Signal received, starting graceful shutdown");
}
