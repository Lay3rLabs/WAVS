#![allow(warnings)]
mod e2e {
    mod clients;
    mod config;
    mod cosmos;
    mod digests;
    mod eth;
    mod handles;
    mod matrix;
    mod runner;
    mod services;

    use config::Configs;
    use digests::Digests;
    use handles::AppHandles;
    use matrix::TestMatrix;
    use services::Services;
    use tracing_subscriber::EnvFilter;
    use utils::context::AppContext;

    #[test]
    fn e2e_tests() {
        if dotenvy::dotenv().is_err() {
            eprintln!("Failed to load .env file");
        }

        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();

        let test_matrix = TestMatrix::new();

        let ctx = AppContext::new();

        let (mut eth_chains, mut cosmos_chains) = (
            eth::start_chains(ctx.clone()),
            cosmos::start_chains(ctx.clone()),
        );

        let configs = Configs::new(
            eth_chains
                .iter()
                .map(|(chain_config, _)| chain_config.clone())
                .collect(),
            cosmos_chains
                .iter()
                .map(|(chain_config, _)| chain_config.clone())
                .collect(),
        );

        let handles = AppHandles::start(
            &ctx,
            &configs,
            eth_chains.drain(..).map(|(_, handle)| handle).collect(),
            cosmos_chains.drain(..).map(|(_, handle)| handle).collect(),
        );

        let clients = clients::Clients::new(ctx.clone(), &configs);

        let digests = Digests::new(ctx.clone(), &clients.http_client, &test_matrix);

        let services = Services::new(ctx.clone(), &configs, &clients, &digests, &test_matrix);

        runner::run_tests(ctx.clone(), clients, services);

        ctx.kill();
        handles.join();
    }
}
