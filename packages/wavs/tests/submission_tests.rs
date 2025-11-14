#![cfg(feature = "dev")]
use std::time::Duration;

use wavs::subsystems::submission::{
    chain_message::{ChainMessage, ChainMessageDebug},
    SubmissionCommand, SubmissionManager,
};
use wavs_types::{
    Component, ComponentDigest, ComponentSource, Credential, Envelope, Service, ServiceManager,
    SignatureKind, Submit, Trigger, Workflow,
};

use utils::{
    context::AppContext, storage::db::WavsDb, telemetry::SubmissionMetrics,
    test_utils::address::rand_address_evm,
};

mod wavs_systems;
use wavs_systems::mock_submissions::{
    mock_event_id, mock_event_order, wait_for_submission_messages,
};

fn dummy_message(service: &Service, payload: &str) -> ChainMessage {
    ChainMessage {
        workflow_id: service.workflows.keys().next().unwrap().clone(),
        service_id: service.id(),
        envelope: Envelope {
            payload: payload.as_bytes().to_vec().into(),
            eventId: mock_event_id().into(),
            ordering: mock_event_order().into(),
        },
        submit: service.workflows.values().next().unwrap().submit.clone(),
        debug: ChainMessageDebug {
            do_not_submit_aggregator: true,
        },
        trigger_data: wavs_types::TriggerData::default(),
    }
}

#[test]
fn collect_messages_with_wait() {
    let config = wavs::config::Config {
        submission_mnemonic: Some(Credential::new(
            "test test test test test test test test test test test junk".to_string(),
        )),
        ..wavs::config::Config::default()
    };
    let meter = opentelemetry::global::meter("wavs_metrics");
    let metrics = SubmissionMetrics::new(meter);
    let services = wavs::services::Services::new(WavsDb::new().unwrap());
    let (dispatcher_to_submission_tx, dispatcher_to_submission_rx) =
        crossbeam::channel::unbounded::<SubmissionCommand>();
    let submission_manager = SubmissionManager::new(
        &config,
        metrics,
        services.clone(),
        dispatcher_to_submission_rx,
    )
    .unwrap();

    assert_eq!(submission_manager.get_message_count(), 0);

    let ctx = AppContext::new();
    std::thread::spawn({
        let ctx = ctx.clone();
        let submission_manager = submission_manager.clone();
        move || submission_manager.start(ctx)
    });

    let service = wavs_types::Service {
        name: "serv1".to_string(),
        status: wavs_types::ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain: "evm:anvil".parse().unwrap(),
            address: rand_address_evm(),
        },
        workflows: vec![(
            "workflow-1".parse().unwrap(),
            Workflow {
                trigger: Trigger::Manual,
                component: Component::new(ComponentSource::Digest(ComponentDigest::hash([0; 32]))),
                submit: Submit::Aggregator {
                    url: "http://example.com".to_string(),
                    component: Box::new(Component::new(ComponentSource::Digest(
                        ComponentDigest::hash([0; 32]),
                    ))),
                    signature_kind: SignatureKind::evm_default(),
                },
            },
        )]
        .into_iter()
        .collect(),
    };
    services.save(&service).unwrap();
    ctx.rt.block_on(async {
        submission_manager
            .add_service_key(service.id(), None)
            .unwrap();
    });

    let msg1 = dummy_message(&service, "foo");
    let msg2 = dummy_message(&service, "bar");
    let msg3 = dummy_message(&service, "baz");

    dispatcher_to_submission_tx
        .send(SubmissionCommand::Submit(msg1.clone()))
        .unwrap();
    wait_for_submission_messages(&submission_manager, 1, None).unwrap();

    assert_eq!(
        submission_manager
            .get_debug_packets()
            .into_iter()
            .map(|x| (x.service.id(), x.workflow_id))
            .collect::<Vec<_>>(),
        vec![(msg1.service_id.clone(), msg1.workflow_id.clone())]
    );

    dispatcher_to_submission_tx
        .send(SubmissionCommand::Submit(msg2.clone()))
        .unwrap();
    dispatcher_to_submission_tx
        .send(SubmissionCommand::Submit(msg3.clone()))
        .unwrap();
    wait_for_submission_messages(&submission_manager, 3, None).unwrap();
    assert_eq!(submission_manager.get_message_count(), 3);
    assert_eq!(
        submission_manager
            .get_debug_packets()
            .into_iter()
            .map(|x| (x.service.id(), x.workflow_id))
            .collect::<Vec<_>>(),
        vec![
            (msg1.service_id.clone(), msg1.workflow_id.clone()),
            (msg2.service_id.clone(), msg2.workflow_id.clone()),
            (msg3.service_id.clone(), msg3.workflow_id.clone()),
        ]
    );

    // show this doesn't loop forever if the 4th never appears
    wait_for_submission_messages(&submission_manager, 4, Some(Duration::from_millis(300)))
        .unwrap_err();
}
