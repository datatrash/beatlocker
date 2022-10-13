use beatlocker_server::{
    enable_default_tracing, App, AppResult, DatabaseOptions, ServerOptions, SubsonicAuth,
    SERVER_VERSION,
};
use clap::Parser;
use std::path::PathBuf;
use tokio::{signal, task};
use tracing::info;

#[derive(Parser)]
#[clap(
    name = "Beatlocker",
    author = "datatrash",
    version = SERVER_VERSION,
)]
struct Cli {
    /// Path to audio library
    #[arg(long)]
    library_path: String,

    /// Path to a data folder Beatlocker may use
    #[arg(long, default_value = ".")]
    data_path: String,

    /// Discogs API token
    #[arg(long, env = "DISCOGS_TOKEN")]
    discogs_token: Option<String>,

    /// Run fully in-memory (no SQLite database will be created)
    #[arg(long)]
    run_in_memory: bool,

    /// Username to use for authentication
    #[arg(long, requires = "auth_password")]
    auth_user: Option<String>,

    /// Password to use for authentication
    #[arg(long, requires = "auth_user")]
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
        subsonic_auth,
        ..Default::default()
    };
    let app = App::new(options).await?;
    let server = axum::Server::bind(&"0.0.0.0:2222".parse().unwrap())
        .serve(app.app.clone().into_make_service())
        .with_graceful_shutdown(shutdown_signal());

    info!("Server started");

    let mgr = app.task_manager.clone();
    let tasks = vec![app.import_all_folders()?, app.import_external_metadata()?];
    let join = task::spawn(async move {
        for task in tasks {
            let _ = mgr.send(task).await;
        }
    });

    server.await?;

    app.task_manager.shutdown().await?;
    join.await?;

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
