use std::time::Duration;

use tokio::sync::mpsc;
use wavs::subsystems::submission::{chain_message::ChainMessage, SubmissionManager};
use wavs_types::{ChainName, Envelope, ServiceId, ServiceManager, Submit};

use utils::{
    context::AppContext, storage::db::RedbStorage, telemetry::SubmissionMetrics,
    test_utils::address::rand_address_evm,
};

mod wavs_systems;
use wavs_systems::mock_submissions::{
    mock_event_id, mock_event_order, wait_for_submission_messages,
};

fn dummy_message(service_id: ServiceId, payload: &str) -> ChainMessage {
    let workflow_id = {
        // whatever, just use the first 24 chars of the service ID
        let mut s = service_id.to_string();
        s.truncate(24);
        s.parse().unwrap()
    };
    ChainMessage {
        workflow_id,
        service_id,
        envelope: Envelope {
            payload: payload.as_bytes().to_vec().into(),
            eventId: mock_event_id().into(),
            ordering: mock_event_order().into(),
        },
        submit: Submit::None,
    }
}

#[test]
fn collect_messages_with_wait() {
    let config = wavs::config::Config {
        submission_mnemonic: Some(
            "test test test test test test test test test test test junk".to_string(),
        ),
        ..wavs::config::Config::default()
    };
    let meter = opentelemetry::global::meter("wavs_metrics");
    let metrics = SubmissionMetrics::new(&meter);
    let data_dir = tempfile::tempdir().unwrap();
    let data_dir = data_dir.path().join("db");
    let services = wavs::services::Services::new(RedbStorage::new(data_dir).unwrap());
    let submission_manager = SubmissionManager::new(&config, metrics, services.clone()).unwrap();

    assert_eq!(submission_manager.get_message_count(), 0);

    let ctx = AppContext::new();
    let (send, rx) = mpsc::channel::<ChainMessage>(2);
    submission_manager.start(ctx.clone(), rx).unwrap();

    let service = wavs_types::Service {
        name: "serv1".to_string(),
        status: wavs_types::ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain_name: ChainName::new("evm").unwrap(),
            address: rand_address_evm(),
        },
        workflows: Default::default(),
    };
    services.save(&service).unwrap();
    ctx.rt.block_on(async {
        submission_manager
            .add_service_key(service.id(), None)
            .unwrap();
    });

    let msg1 = dummy_message(service.id(), "foo");
    let msg2 = dummy_message(service.id(), "bar");
    let msg3 = dummy_message(service.id(), "baz");

    send.blocking_send(msg1.clone()).unwrap();
    wait_for_submission_messages(&submission_manager, 1, None).unwrap();

    assert_eq!(
        submission_manager
            .get_debug_packets()
            .into_iter()
            .map(|x| (x.service.id(), x.workflow_id))
            .collect::<Vec<_>>(),
        vec![(msg1.service_id.clone(), msg1.workflow_id.clone())]
    );

    send.blocking_send(msg2.clone()).unwrap();
    send.blocking_send(msg3.clone()).unwrap();
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
