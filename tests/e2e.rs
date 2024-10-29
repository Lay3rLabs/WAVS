mod helpers;

#[cfg(feature = "e2e_tests")]
mod e2e {
    use super::helpers;

    use std::{path::PathBuf, sync::Arc};

    use helpers::app::TestApp;
    use layer_climb::prelude::*;
    use wasmatic::{context::AppContext, dispatcher::CoreDispatcher};

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

        let ctx = AppContext::new(config.clone());

        let dispatcher = Arc::new(CoreDispatcher::new_core(ctx.clone()).unwrap());

        let wasmatic_handle = std::thread::spawn({
            let dispatcher = dispatcher.clone();
            let ctx = ctx.clone();
            move || {
                wasmatic::start(ctx, dispatcher);
            }
        });

        let test_handle = std::thread::spawn({
            let ctx = ctx.clone();
            move || {
                ctx.rt.clone().block_on({
                    async move {
                        run_tests(ctx.clone()).await;
                        ctx.kill();
                    }
                });
            }
        });

        wasmatic_handle.join().unwrap();
        test_handle.join().unwrap();
    }

    async fn run_tests(ctx: AppContext) {
        let query_client = QueryClient::new(ctx.config.chain_config().unwrap())
            .await
            .unwrap();
        tracing::info!("TODO - run tests on {}", query_client.chain_config.chain_id);
        tracing::info!("Sleeping for 1 second...");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
