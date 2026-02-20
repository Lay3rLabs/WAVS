#![cfg(feature = "dev")]
use std::time::Duration;

use wavs::subsystems::submission::SubmissionCommand;

use utils::{context::AppContext, telemetry::Metrics};

mod wavs_systems;
use wavs_systems::mock_submissions::wait_for_submission_messages;

use crate::wavs_systems::{
    channels::TestChannels,
    mock_config::mock_config,
    mock_service::{mock_service, mock_services},
    mock_submissions::{mock_submission_manager, mock_submission_request},
};

#[test]
fn collect_messages_with_wait() {
    let ctx = AppContext::new();
    let channels = TestChannels::new();
    let services = mock_services();
    let service = mock_service();
    let metrics = Metrics::new(opentelemetry::global::meter("wavs_metrics"));

    services.save(&service).unwrap();

    let submission_manager =
        mock_submission_manager(ctx.clone(), &metrics, &mock_config(), &channels, services);

    ctx.rt.block_on(async {
        submission_manager
            .add_service_key(service.id(), None)
            .unwrap();
    });

    let msg1 = mock_submission_request(&service, "foo");
    let msg2 = mock_submission_request(&service, "bar");
    let msg3 = mock_submission_request(&service, "baz");

    channels
        .dispatcher_to_submission_tx
        .send(SubmissionCommand::Submit(msg1.clone()))
        .unwrap();
    wait_for_submission_messages(&submission_manager, 1, None).unwrap();

    assert_eq!(
        submission_manager
            .get_debug_submissions()
            .into_iter()
            .map(|x| (x.service_id().clone(), x.workflow_id().clone()))
            .collect::<Vec<_>>(),
        vec![(msg1.service_id().clone(), msg1.workflow_id().clone())]
    );

    channels
        .dispatcher_to_submission_tx
        .send(SubmissionCommand::Submit(msg2.clone()))
        .unwrap();
    channels
        .dispatcher_to_submission_tx
        .send(SubmissionCommand::Submit(msg3.clone()))
        .unwrap();
    wait_for_submission_messages(&submission_manager, 3, None).unwrap();
    assert_eq!(submission_manager.metrics.get_dispatch_count(), 3);
    assert_eq!(
        submission_manager
            .get_debug_submissions()
            .into_iter()
            .map(|x| (x.service_id().clone(), x.workflow_id().clone()))
            .collect::<Vec<_>>(),
        vec![
            (msg1.service_id().clone(), msg1.workflow_id().clone()),
            (msg2.service_id().clone(), msg2.workflow_id().clone()),
            (msg3.service_id().clone(), msg3.workflow_id().clone()),
        ]
    );

    // show this doesn't loop forever if the 4th never appears
    wait_for_submission_messages(&submission_manager, 4, Some(Duration::from_millis(300)))
        .unwrap_err();
}
