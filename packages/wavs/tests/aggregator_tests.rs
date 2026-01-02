#![cfg(feature = "dev")]

mod wavs_systems;

use utils::{context::AppContext, telemetry::Metrics};
use wavs::subsystems::aggregator::AggregatorCommand;

use crate::wavs_systems::{
    channels::TestChannels,
    mock_aggregator::{mock_aggregator, wait_for_aggregator_broadcasts},
    mock_config::mock_config,
    mock_service::{mock_service, mock_services},
    mock_submissions::{mock_submission_manager, mock_submission_request},
};

#[test]
fn single() {
    let ctx = AppContext::new();
    let channels = TestChannels::new();
    let services = mock_services();
    let service = mock_service();
    let metrics = Metrics::new(opentelemetry::global::meter("wavs_metrics"));
    let config = mock_config();

    services.save(&service).unwrap();

    let submission_manager =
        mock_submission_manager(ctx.clone(), &metrics, &config, &channels, services.clone());

    let aggregator = mock_aggregator(ctx.clone(), &metrics, &config, &channels, services);

    ctx.rt.block_on(async {
        submission_manager
            .add_service_key(service.id(), None)
            .unwrap();
    });

    let mut submissions = Vec::new();

    for i in 0..3 {
        let req = mock_submission_request(&service, &format!("message {i}"));
        ctx.rt.block_on(async {
            submissions.push(submission_manager.sign_request(&req).await.unwrap());
        });
    }

    for submission in &submissions {
        channels
            .dispatcher_to_aggregator_tx
            .send(AggregatorCommand::Broadcast(submission.clone()))
            .unwrap();
    }

    wait_for_aggregator_broadcasts(&aggregator, 3, None).unwrap();
    assert_eq!(aggregator.metrics.get_broadcast_count(), 3);
}
