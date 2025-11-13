#![cfg(feature = "dev")]
use std::collections::BTreeMap;

use utils::storage::db::{Table, TableHandle, WavsDb};
use utils::test_utils::address::rand_address_evm;
use wavs_types::{
    Component, ComponentDigest, ComponentSource, Service, ServiceId, ServiceManager, ServiceStatus,
    Submit, Workflow, WorkflowId,
};

use serde::{Deserialize, Serialize};
mod wavs_systems;
use wavs_systems::mock_trigger_manager::mock_evm_event_trigger;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct Demo {
    pub name: String,
    pub age: u16,
    pub nicknames: Vec<String>,
}

// basic types - using test tables
const T1: TableHandle<u32, String> = TableHandle::new(Table::Test("t1"));

// json types with &str key - using test tables
const TJ: TableHandle<String, Demo> = TableHandle::new(Table::Test("tj"));

#[test]
fn test_set_once_and_get() {
    let store = WavsDb::new().unwrap();

    let empty = store.get(&T1, 17).unwrap();
    assert!(empty.is_none());

    let data = "hello".to_string();
    store.set(&T1, 17, data.clone()).unwrap();
    let full = store.get(&T1, 17).unwrap().unwrap();
    assert_eq!(data, full);
}

#[test]
fn test_json_storage() {
    let store = WavsDb::new().unwrap();

    let empty = store.get(&TJ, "john".to_string()).unwrap();
    assert!(empty.is_none());

    let data = Demo {
        name: "John".to_string(),
        age: 28,
        nicknames: vec!["Johnny".to_string(), "Mr. Rocket".to_string()],
    };
    store.set(&TJ, "john".to_string(), data.clone()).unwrap();
    let full = store.get(&TJ, "john".to_string()).unwrap().unwrap();
    assert_eq!(data, full);
}

#[test]
fn db_service_store() {
    let storage = WavsDb::new().unwrap();

    const SERVICE_TABLE: TableHandle<[u8; 32], Service> =
        TableHandle::new(Table::Test("service_table"));

    let workflows: BTreeMap<WorkflowId, Workflow> = [
        (
            WorkflowId::new("workflow-id-1").unwrap(),
            Workflow {
                trigger: mock_evm_event_trigger(),
                component: Component::new(ComponentSource::Digest(ComponentDigest::hash(
                    b"digest-1",
                ))),
                submit: Submit::None,
            },
        ),
        (
            WorkflowId::new("workflow-id-2").unwrap(),
            Workflow {
                trigger: mock_evm_event_trigger(),
                component: Component::new(ComponentSource::Digest(ComponentDigest::hash(
                    b"digest-2",
                ))),
                submit: Submit::None,
            },
        ),
    ]
    .into();

    let service = Service {
        name: "service-id-1".to_string(),
        workflows,
        status: ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain: "evm:anvil".parse().unwrap(),
            address: rand_address_evm(),
        },
    };

    storage
        .set(&SERVICE_TABLE, service.id().inner(), service.clone())
        .unwrap();

    let service_stored = storage
        .get(&SERVICE_TABLE, service.id().inner())
        .unwrap()
        .unwrap();

    let expected_service_serialized = serde_json::to_vec(&service).unwrap();
    let service_stored_serialized = serde_json::to_vec(&service_stored).unwrap();
    assert_eq!(expected_service_serialized, service_stored_serialized);

    // can read keys via iterator
    let keys = storage
        .with_table_read(&SERVICE_TABLE, |table| {
            let mut keys = Vec::new();
            for entry in table.iter() {
                let (key_bytes, _) = entry.pair();
                keys.push(ServiceId::from(*key_bytes));
            }
            Ok(keys)
        })
        .unwrap();

    assert_eq!(vec![service.id()], keys);

    let values = storage
        .with_table_read(&SERVICE_TABLE, |table| {
            Ok(table
                .iter()
                .map(|entry| entry.pair().1.clone())
                .collect::<Vec<Service>>())
        })
        .unwrap();

    let values_serialized = values
        .into_iter()
        .map(|service| serde_json::to_vec(&service).unwrap())
        .collect::<Vec<Vec<u8>>>();

    assert_eq!(vec![expected_service_serialized], values_serialized);
}
