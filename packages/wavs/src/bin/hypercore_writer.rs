use std::{io::Write, path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;
use hypercore::{HypercoreBuilder, Storage};
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::Mutex;
use tokio_util::compat::TokioAsyncReadCompatExt;

use wavs::subsystems::trigger::streams::hypercore_protocol;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    storage_dir: PathBuf,
    #[arg(long, default_value = "127.0.0.1:0")]
    listen: String,
    #[arg(long)]
    append_data: Option<String>,
    #[arg(long, default_value_t = 0)]
    append_after_ms: u64,
    #[arg(long, default_value_t = false)]
    overwrite: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    std::fs::create_dir_all(&args.storage_dir)?;

    let storage = Storage::new_disk(&args.storage_dir, args.overwrite).await?;
    let core = HypercoreBuilder::new(storage).build().await?;
    let core = Arc::new(Mutex::new(core));
    let feed_key = {
        let core = core.lock().await;
        core.key_pair().public.to_bytes()
    };
    println!("FEED_KEY={}", const_hex::encode(feed_key));
    let _ = std::io::stdout().flush();

    if let Some(data) = args.append_data.clone() {
        let core = Arc::clone(&core);
        tokio::spawn(async move {
            if args.append_after_ms > 0 {
                tokio::time::sleep(Duration::from_millis(args.append_after_ms)).await;
            }
            let mut core = core.lock().await;
            if let Err(err) = core.append(data.as_bytes()).await {
                eprintln!("append failed: {err:?}");
            }
        });
    }

    if let Some(path) = args.listen.strip_prefix("unix:") {
        let path = path.trim_start_matches("//");
        let socket_path = PathBuf::from(path);
        if socket_path.exists() {
            std::fs::remove_file(&socket_path)?;
        }
        let listener = UnixListener::bind(&socket_path)?;
        loop {
            let (stream, _) = listener.accept().await?;
            let core = Arc::clone(&core);
            tokio::spawn(async move {
                if let Err(err) =
                    hypercore_protocol::run_protocol(stream.compat(), false, core, feed_key).await
                {
                    tracing::warn!("Hypercore protocol writer error: {err:?}");
                }
            });
        }
    } else {
        let listener = TcpListener::bind(&args.listen).await?;
        loop {
            let (stream, _) = listener.accept().await?;
            let core = Arc::clone(&core);
            tokio::spawn(async move {
                if let Err(err) =
                    hypercore_protocol::run_protocol(stream.compat(), false, core, feed_key).await
                {
                    tracing::warn!("Hypercore protocol writer error: {err:?}");
                }
            });
        }
    }
}
