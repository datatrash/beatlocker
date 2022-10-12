use beatlocker_server::{
    enable_default_tracing, App, AppResult, DatabaseOptions, ServerOptions, TaskMessage,
    SERVER_VERSION, USER_AGENT,
};
use clap::Parser;
use std::path::PathBuf;
use tokio::signal;
use tracing::info;

#[derive(Parser)]
#[clap(
    name = "Beatlocker",
    author = "datatrash",
    version = SERVER_VERSION
)]
struct Cli {
    /// Path to audio library
    #[clap(long)]
    library_path: String,

    /// Path to a data folder Beatlocker may use
    #[clap(long, default_value = ".")]
    data_path: String,

    /// Discogs API token
    #[clap(long, env = "DISCOGS_TOKEN")]
    discogs_token: Option<String>,

    /// Run fully in-memory (no SQLite database will be created)
    #[clap(long)]
    run_in_memory: bool,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    let cli = Cli::parse();

    enable_default_tracing();

    info!("beatlocker {}", SERVER_VERSION);
    info!("Server starting...");

    musicbrainz_rs::config::set_user_agent(USER_AGENT);

    let options = ServerOptions {
        path: PathBuf::from(cli.library_path),
        database: DatabaseOptions {
            path: Some(PathBuf::from(cli.data_path)),
            in_memory: cli.run_in_memory,
        },
        server_version: SERVER_VERSION.to_string(),
        include_cover_art: true,
        discogs_token: cli.discogs_token,
        ..Default::default()
    };
    let app = App::new(options).await?;
    let server = axum::Server::bind(&"0.0.0.0:2222".parse().unwrap())
        .serve(app.app.clone().into_make_service())
        .with_graceful_shutdown(shutdown_signal());

    let (task_tx, done_rx) = app.start_background_tasks().await?;

    info!("Server started");
    server.await?;

    task_tx.send(TaskMessage::Shutdown).await?;
    done_rx.await?;

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
