mod helpers;

#[cfg(feature = "e2e_tests")]
mod e2e {
    use super::helpers;

    use std::{path::PathBuf, sync::Arc};

    use helpers::app::TestApp;
    use layer_climb::prelude::*;
    use wasmatic::{config::Config, context::AppContext, dispatcher::CoreDispatcher};

    #[test]
    fn e2e_tests() {
        let config = {
            tokio::runtime::Runtime::new().unwrap().block_on({
                async {
                    let mut cli_args = TestApp::default_cli_args();
                    cli_args.data = Some(
                        PathBuf::from(file!())
                            .parent()
                            .unwrap()
                            .join("wasmatic")
                            .join("test-data"),
                    );
                    TestApp::new_with_args(cli_args)
                        .await
                        .config
                        .as_ref()
                        .clone()
                }
            })
        };

        let ctx = AppContext::new();

        let dispatcher = Arc::new(CoreDispatcher::new_core(&config).unwrap());

        let (wasmatic_end_sender, wasmatic_end_receiver) = tokio::sync::oneshot::channel::<()>();

        let wasmatic_handle = std::thread::spawn({
            let dispatcher = dispatcher.clone();
            let ctx = ctx.clone();
            let config = config.clone();
            move || {
                wasmatic::run_server(ctx, config, dispatcher);
                wasmatic_end_sender.send(()).unwrap();
            }
        });

        let test_handle = std::thread::spawn({
            move || {
                ctx.rt.clone().block_on({
                    async move {
                        run_tests(config).await;
                        ctx.kill_switch.kill();

                        tokio::select! {
                            _ = wasmatic_end_receiver => {},
                            _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {
                                panic!("Wasmatic did not kill properly");
                            }
                        }
                    }
                });
            }
        });

        // this must come first so we can catch the timeout panic
        test_handle.join().unwrap();
        wasmatic_handle.join().unwrap();
    }

    async fn run_tests(config: Config) {
        let query_client = QueryClient::new(config.chain_config().unwrap())
            .await
            .unwrap();
        tracing::info!("TODO - run tests on {}", query_client.chain_config.chain_id);
        tracing::info!("Sleeping for 1 second...");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
