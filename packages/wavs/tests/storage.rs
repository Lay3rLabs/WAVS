#![cfg(feature = "dev")]
use std::collections::BTreeMap;

use utils::storage::db::{WavsDb, WavsDbTable};
use utils::test_utils::address::rand_address_evm;
use wavs_types::{
    Component, ComponentDigest, ComponentSource, Service, ServiceManager, ServiceStatus, Submit,
    Workflow, WorkflowId,
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

#[test]
fn test_wavsdb_table_basic_operations() {
    let table: WavsDbTable<u32, String> = WavsDbTable::new("test").unwrap();

    let empty = table.get_cloned(&17);
    assert!(empty.is_none());

    let data = "hello".to_string();
    table.insert(17, data.clone()).unwrap();
    let full = table.get_cloned(&17).unwrap();
    assert_eq!(data, full);
}

#[test]
fn test_wavsdb_table_json_storage() {
    let table: WavsDbTable<String, Demo> = WavsDbTable::new("test").unwrap();

    let empty = table.get_cloned(&"john".to_string());
    assert!(empty.is_none());

    let data = Demo {
        name: "John".to_string(),
        age: 28,
        nicknames: vec!["Johnny".to_string(), "Mr. Rocket".to_string()],
    };
    table.insert("john".to_string(), data.clone()).unwrap();
    let full = table.get_cloned(&"john".to_string()).unwrap();
    assert_eq!(data, full);
}

#[test]
fn db_service_store() {
    let storage = WavsDb::new().unwrap();

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
        .services
        .insert(service.id(), service.clone())
        .unwrap();

    let service_stored = storage.services.get_cloned(&service.id()).unwrap();

    let expected_service_serialized = serde_json::to_vec(&service).unwrap();
    let service_stored_serialized = serde_json::to_vec(&service_stored).unwrap();
    assert_eq!(expected_service_serialized, service_stored_serialized);

    // can read keys via iterator
    let mut keys = Vec::new();
    for entry in storage.services.iter() {
        let (service_id, _) = entry.pair();
        keys.push(service_id.clone());
    }

    assert_eq!(vec![service.id()], keys);

    let values: Vec<Service> = storage
        .services
        .iter()
        .map(|entry| entry.pair().1.clone())
        .collect();

    let values_serialized = values
        .into_iter()
        .map(|service| serde_json::to_vec(&service).unwrap())
        .collect::<Vec<Vec<u8>>>();

    assert_eq!(vec![expected_service_serialized], values_serialized);
}

#[test]
fn test_kv_operations() {
    let storage = WavsDb::new().unwrap();

    // Test kv_store operations
    let key = "test_key".to_string();
    let value = b"test_value".to_vec();

    assert!(storage.kv_store.get_cloned(&key).is_none());
    storage.kv_store.insert(key.clone(), value.clone()).unwrap();

    let retrieved = storage.kv_store.get_cloned(&key).unwrap();
    assert_eq!(retrieved, value);

    // Test kv_atomics_counter operations
    let counter_key = "counter".to_string();
    let counter_value = 42i64;

    assert!(storage
        .kv_atomics_counter
        .get_cloned(&counter_key)
        .is_none());
    storage
        .kv_atomics_counter
        .insert(counter_key.clone(), counter_value)
        .unwrap();

    let retrieved_counter = storage.kv_atomics_counter.get_cloned(&counter_key).unwrap();
    assert_eq!(retrieved_counter, counter_value);
}
