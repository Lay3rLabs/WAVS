use std::collections::BTreeMap;

use wavs::{
    apis::{
        dispatcher::{Component, Service, ServiceStatus, Workflow},
        trigger::Trigger,
        ComponentID, ServiceID, WorkflowID,
    },
    storage::db::{RedbStorage, Table, JSON},
    test_utils::address::rand_address_eth,
    Digest,
};

use redb::ReadableTable;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct Demo {
    pub name: String,
    pub age: u16,
    pub nicknames: Vec<String>,
}

// basic types
const T1: Table<u32, String> = Table::new("t1");

// json types with &str key
const TJ: Table<&str, JSON<Demo>> = Table::new("tj");

#[test]
fn test_set_once_and_get() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let store = RedbStorage::new(file.path()).unwrap();

    let empty = store.get(T1, 17).unwrap();
    assert!(empty.is_none());

    let data = "hello".to_string();
    store.set(T1, 17, &data).unwrap();
    let full = store.get(T1, 17).unwrap().unwrap();
    assert_eq!(data, full.value());
}

#[test]
fn test_json_storage() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let store = RedbStorage::new(file.path()).unwrap();

    let empty = store.get(TJ, "john").unwrap();
    assert!(empty.is_none());

    let data = Demo {
        name: "John".to_string(),
        age: 28,
        nicknames: vec!["Johnny".to_string(), "Mr. Rocket".to_string()],
    };
    store.set(TJ, "john", &data).unwrap();
    let full = store.get(TJ, "john").unwrap().unwrap();
    assert_eq!(data, full.value());
}

#[test]
fn db_service_store() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let storage = RedbStorage::new(file.path()).unwrap();

    const SERVICE_TABLE: Table<&str, JSON<Service>> = Table::new("temp-services");

    let service_id = ServiceID::new("service-id-1").unwrap();

    let components: BTreeMap<ComponentID, Component> = [
        (
            ComponentID::new("component-id-1").unwrap(),
            Component::new(&Digest::new(b"digest-1")),
        ),
        (
            ComponentID::new("component-id-2").unwrap(),
            Component::new(&Digest::new(b"digest-2")),
        ),
    ]
    .into();

    let workflows: BTreeMap<WorkflowID, Workflow> = [
        (
            WorkflowID::new("workflow-id-1").unwrap(),
            Workflow {
                trigger: Trigger::eth_event(rand_address_eth()),
                component: ComponentID::new("component-id-1").unwrap(),
                submit: None,
            },
        ),
        (
            WorkflowID::new("workflow-id-2").unwrap(),
            Workflow {
                trigger: Trigger::eth_event(rand_address_eth()),
                component: ComponentID::new("component-id-2").unwrap(),
                submit: None,
            },
        ),
    ]
    .into();

    let service = Service {
        id: service_id.clone(),
        name: service_id.to_string(),
        components,
        workflows,
        status: ServiceStatus::Active,
        testable: true,
    };

    storage.set(SERVICE_TABLE, &service_id, &service).unwrap();

    let service_stored = storage.get(SERVICE_TABLE, &service_id).unwrap().unwrap();

    assert_eq!(service, service_stored.value());

    // can read keys via iterator
    let keys = storage
        .map_table_read(SERVICE_TABLE, |table| {
            Ok(table
                .unwrap()
                .iter()
                .unwrap()
                .map(|entry| {
                    let (k, _) = entry.unwrap();
                    k.value().to_string()
                })
                .collect::<Vec<String>>())
        })
        .unwrap();

    assert_eq!(vec![service_id.to_string()], keys);

    let values = storage
        .map_table_read(SERVICE_TABLE, |table| {
            Ok(table
                .unwrap()
                .iter()
                .unwrap()
                .map(|entry| {
                    let (_, v) = entry.unwrap();
                    v.value()
                })
                .collect::<Vec<Service>>())
        })
        .unwrap();

    assert_eq!(vec![service], values);
}
