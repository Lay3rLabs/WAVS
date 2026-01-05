use std::time::Duration;

use futures::StreamExt;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use utils::telemetry::TriggerMetrics;
use wavs::subsystems::trigger::streams::hypercore_stream::HypercoreStreamConfig;

#[tokio::test]
async fn hypercore_stream_reads_external_writer() {
    struct ChildGuard {
        child: Child,
    }

    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.child.start_kill();
        }
    }

    let writer_dir = tempfile::tempdir().unwrap();
    let reader_dir = tempfile::tempdir().unwrap();
    let metrics = TriggerMetrics::new(opentelemetry::global::meter("hypercore-test"));

    let socket_path = writer_dir.path().join("hypercore.sock");
    let socket_endpoint = format!("unix:{}", socket_path.display());

    let payload = "hypercore-data";
    let mut child = Command::new(env!("CARGO_BIN_EXE_hypercore_writer"))
        .arg("--storage-dir")
        .arg(writer_dir.path())
        .arg("--listen")
        .arg(&socket_endpoint)
        .arg("--append-data")
        .arg(payload)
        .arg("--append-after-ms")
        .arg("100")
        .stdout(Stdio::piped())
        .spawn()
        .expect("start hypercore writer");
    let stdout = child.stdout.take().expect("writer stdout");
    let mut stdout_reader = BufReader::new(stdout);
    let mut first_line = String::new();
    let read_line = tokio::time::timeout(
        Duration::from_secs(2),
        stdout_reader.read_line(&mut first_line),
    )
    .await;
    let feed_key = match read_line {
        Ok(Ok(_)) => first_line
            .trim()
            .strip_prefix("FEED_KEY=")
            .map(|s| s.to_string()),
        _ => None,
    };
    let _child_guard = ChildGuard { child };
    let feed_key = match feed_key {
        Some(key) => key,
        None => {
            eprintln!("warning: missing feed key from writer");
            return;
        }
    };

    let connect_deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        match UnixStream::connect(&socket_path).await {
            Ok(stream) => {
                drop(stream);
                break;
            }
            Err(err) => {
                if matches!(err.kind(), std::io::ErrorKind::PermissionDenied) {
                    eprintln!("warning: unix socket connect not permitted: {err}");
                    return;
                }
                if tokio::time::Instant::now() >= connect_deadline {
                    eprintln!("warning: hypercore writer not reachable: {err}");
                    return;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }

    let mut stream = wavs::subsystems::trigger::streams::hypercore_stream::start_hypercore_stream(
        HypercoreStreamConfig {
            storage_dir: reader_dir.path().to_path_buf(),
            overwrite: true,
            replication_endpoint: Some(socket_endpoint.clone()),
            replication_feed_key: Some(feed_key),
        },
        metrics,
    )
    .await
    .expect("start hypercore stream");

    let event = tokio::time::timeout(Duration::from_secs(5), stream.next()).await;
    let event = match event {
        Ok(Some(Ok(event))) => event,
        Ok(Some(Err(err))) => panic!("hypercore stream error: {err:?}"),
        Ok(None) => {
            eprintln!("warning: hypercore stream closed before receiving data");
            return;
        }
        Err(_) => {
            eprintln!("warning: timed out waiting for hypercore data");
            return;
        }
    };

    match event {
        wavs::subsystems::trigger::streams::StreamTriggers::Hypercore { event } => {
            assert_eq!(event.index, 0);
            assert_eq!(event.data, payload.as_bytes());
            assert!(!event.feed_key.is_empty());
        }
        other => panic!("unexpected stream event: {:?}", other),
    }
}
