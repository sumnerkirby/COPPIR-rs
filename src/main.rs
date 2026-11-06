//! COPPIR launcher: start the local server, then open the desktop window
//! (or, with `--headless`, just serve and print the address).

use std::path::PathBuf;

use clap::Parser;
use coppir::{Config, config};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about = "Common Operational Picture Program for Incident Response")]
struct Cli {
    /// Port to listen on, on 127.0.0.1.
    #[arg(long, env = "COPPIR_PORT", default_value_t = 8000)]
    port: u16,

    /// Fail instead of picking another port when the requested one is taken.
    #[arg(long)]
    strict_port: bool,

    /// Where version 1 saved scenarios, to offer them for import. Defaults to
    /// `scenarios` in the per-user data directory.
    #[arg(long, env = "COPPIR_SCENARIOS_DIR")]
    scenarios_dir: Option<PathBuf>,

    /// The exercise database. Defaults to `coppir.db` in the per-user data directory.
    #[arg(long, env = "COPPIR_DATABASE")]
    database: Option<PathBuf>,

    /// Serve only; open the printed URL in a browser yourself.
    #[arg(long)]
    headless: bool,
}

fn main() -> anyhow_lite::Result {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("coppir=info,warn")))
        .init();

    let cli = Cli::parse();
    let scenarios_dir = cli.scenarios_dir.unwrap_or_else(config::default_scenarios_dir);
    let mut config = Config::new(scenarios_dir);
    config.database = Some(cli.database.unwrap_or_else(|| config::data_dir().join("coppir.db")));

    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let listener = runtime.block_on(coppir::bind(cli.port, !cli.strict_port))?;
    let url = format!("http://{}", listener.local_addr()?);
    if let Some(db) = &config.database {
        tracing::info!("database at {}", db.display());
    }

    if cli.headless || !cfg!(feature = "desktop") {
        tracing::info!("serving on {url}");
        println!("COPPIR is running at {url}  (Ctrl+C to stop)");
        runtime.block_on(coppir::serve(listener, config, shutdown_signal()))?;
        return Ok(());
    }

    #[cfg(feature = "desktop")]
    {
        // The server runs on the runtime's worker threads; the window needs
        // the main thread, and owns the process from here on.
        runtime.spawn(async move {
            if let Err(e) = coppir::serve(listener, config, std::future::pending()).await {
                tracing::error!("server stopped: {e}");
            }
        });
        coppir::desktop::run(&url)?;
    }
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
}

/// Just enough error plumbing for `main` without another dependency.
mod anyhow_lite {
    pub type Result = std::result::Result<(), Box<dyn std::error::Error>>;
}
