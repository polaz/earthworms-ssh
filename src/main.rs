use std::{borrow::Cow, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use clap::Parser;
use russh::{
    Preferred, compression,
    server::{self, Server},
};
use tokio::time::MissedTickBehavior;
use tracing::info;

use worms_ssh::{
    game::{Event, Game, TICK_RATE},
    ssh::WormServer,
};

#[derive(Debug, Parser)]
#[command(
    name = "worms-ssh",
    about = "Realtime ASCII artillery arena served over SSH"
)]
struct Args {
    /// TCP address on which the SSH game accepts players.
    #[arg(long, env = "WORMS_LISTEN", default_value = "0.0.0.0:2222")]
    listen: SocketAddr,

    /// OpenSSH private host key used to identify this SSH server.
    #[arg(long, env = "WORMS_HOST_KEY", default_value = "host_key")]
    host_key: PathBuf,

    /// Deterministic world seed, useful for tournaments and testing.
    #[arg(long, env = "WORMS_SEED")]
    seed: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "worms_ssh=info".into()),
        )
        .init();

    let args = Args::parse();
    let key = russh::keys::load_secret_key(&args.host_key, None)
        .with_context(|| format!("failed to load SSH host key {}", args.host_key.display()))?;
    let config = Arc::new(server::Config {
        keys: vec![key],
        inactivity_timeout: Some(Duration::from_secs(300)),
        auth_rejection_time: Duration::ZERO,
        preferred: compressed_preferences(),
        ..Default::default()
    });

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let mut game = Game::new(args.seed.unwrap_or_else(rand::random));
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_millis(1000 / TICK_RATE));
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            while let Ok(event) = event_rx.try_recv() {
                game.accept(event);
            }
            game.tick();
            game.broadcast();
        }
    });

    info!(address = %args.listen, "ASCII arena accepting SSH players");
    info!("connect with: ssh -p {} <name>@<host>", args.listen.port());
    let mut server = WormServer::new(event_tx);
    server
        .run_on_address(config, args.listen)
        .await
        .context("SSH listener terminated")?;
    Ok(())
}

fn compressed_preferences() -> Preferred {
    Preferred {
        compression: Cow::Borrowed(&[compression::ZLIB_LEGACY, compression::NONE]),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_prefers_delayed_compression_with_uncompressed_fallback() {
        let preferred = compressed_preferences();
        let names: Vec<&str> = preferred.compression.iter().map(AsRef::as_ref).collect();

        assert_eq!(names, ["zlib@openssh.com", "none"]);
    }
}
