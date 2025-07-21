mod helpers;

use std::collections::HashMap;

use crate::helpers::exec::{execute_component, try_execute_component};
use example_types::{KvStoreError, KvStoreRequest, KvStoreResponse};
use utils::{
    init_tracing_tests, storage::db::RedbStorage, test_utils::mock_engine::COMPONENT_KV_STORE_BYTES,
};
use wavs_engine::backend::wasi_keyvalue::context::KeyValueCtx;

#[tokio::test]
async fn keyvalue_basic() {
    init_tracing_tests();

    const BUCKET: &str = "test_bucket";
    const KEY: &str = "test_key";
    const VALUE: &[u8] = b"hello";

    let db_dir = tempfile::tempdir().unwrap();
    let db = RedbStorage::new(db_dir.path()).unwrap();
    let keyvalue_ctx = KeyValueCtx::new(db.clone(), "test".to_string());

    // Write a value to the key-value store
    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::Write {
            bucket: BUCKET.to_string(),
            key: KEY.to_string(),
            value: VALUE.to_vec(),
        },
    )
    .await;

    assert_eq!(resp, KvStoreResponse::Write,);

    // Read it back
    let resp = execute_component::<KvStoreResponse>(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx),
        KvStoreRequest::Read {
            bucket: BUCKET.to_string(),
            key: KEY.to_string(),
        },
    )
    .await;

    assert_eq!(
        resp,
        KvStoreResponse::Read {
            value: VALUE.to_vec()
        },
    );
}

#[tokio::test]
async fn keyvalue_wrong_context() {
    init_tracing_tests();

    const BUCKET: &str = "test_bucket";
    const KEY: &str = "test_key";
    const VALUE: &[u8] = b"hello";

    let db_dir = tempfile::tempdir().unwrap();
    let db = RedbStorage::new(db_dir.path()).unwrap();
    let keyvalue_ctx_1 = KeyValueCtx::new(db.clone(), "test-1".to_string());
    let keyvalue_ctx_2 = KeyValueCtx::new(db.clone(), "test-2".to_string());

    // Write a value to the key-value store
    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx_1),
        KvStoreRequest::Write {
            bucket: BUCKET.to_string(),
            key: KEY.to_string(),
            value: VALUE.to_vec(),
        },
    )
    .await;

    assert_eq!(resp, KvStoreResponse::Write,);

    // Attempt to read the wrong context
    let err = try_execute_component::<KvStoreResponse>(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx_2),
        KvStoreRequest::Read {
            bucket: BUCKET.to_string(),
            key: KEY.to_string(),
        },
    )
    .await
    .unwrap_err();

    assert_eq!(
        err,
        KvStoreError::MissingKey {
            bucket: BUCKET.to_string(),
            key: KEY.to_string(),
        }
        .to_string()
    );
}

#[tokio::test]
async fn keyvalue_wrong_key() {
    init_tracing_tests();

    const BUCKET: &str = "test_bucket";
    const KEY: &str = "test_key";
    const BAD_KEY: &str = "bad_test_key";
    const VALUE: &[u8] = b"hello";

    let db_dir = tempfile::tempdir().unwrap();
    let db = RedbStorage::new(db_dir.path()).unwrap();
    let keyvalue_ctx = KeyValueCtx::new(db.clone(), "test".to_string());

    // Write a value to the key-value store
    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::Write {
            bucket: BUCKET.to_string(),
            key: KEY.to_string(),
            value: VALUE.to_vec(),
        },
    )
    .await;

    assert_eq!(resp, KvStoreResponse::Write,);

    // Attempt to read the wrong key
    let err = try_execute_component::<KvStoreResponse>(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx),
        KvStoreRequest::Read {
            bucket: BUCKET.to_string(),
            key: BAD_KEY.to_string(),
        },
    )
    .await
    .unwrap_err();

    assert_eq!(
        err,
        KvStoreError::MissingKey {
            bucket: BUCKET.to_string(),
            key: BAD_KEY.to_string(),
        }
        .to_string()
    );
}

#[tokio::test]
async fn keyvalue_atomic_increment() {
    init_tracing_tests();

    const BUCKET: &str = "test_bucket";
    const KEY_1: &str = "test_key_1";
    const KEY_2: &str = "test_key_2";

    let db_dir = tempfile::tempdir().unwrap();
    let db = RedbStorage::new(db_dir.path()).unwrap();
    let keyvalue_ctx = KeyValueCtx::new(db.clone(), "test".to_string());

    // Increment the key (without setting it first)
    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::AtomicIncrement {
            bucket: BUCKET.to_string(),
            key: KEY_1.to_string(),
            delta: 3,
        },
    )
    .await;

    assert_eq!(resp, KvStoreResponse::AtomicIncrement { value: 3 });

    // Increment the key again so we can test against previous value
    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::AtomicIncrement {
            bucket: BUCKET.to_string(),
            key: KEY_1.to_string(),
            delta: 2,
        },
    )
    .await;

    assert_eq!(resp, KvStoreResponse::AtomicIncrement { value: 5 });

    // Same process as above, but with a preset key
    // behavior here is currently undefined, but we expect it to be a separate table
    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::Write {
            bucket: BUCKET.to_string(),
            key: KEY_2.to_string(),
            value: 10i64.to_le_bytes().to_vec(),
        },
    )
    .await;
    assert_eq!(resp, KvStoreResponse::Write,);

    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::AtomicIncrement {
            bucket: BUCKET.to_string(),
            key: KEY_2.to_string(),
            delta: 3,
        },
    )
    .await;

    assert_eq!(resp, KvStoreResponse::AtomicIncrement { value: 3 });
}

#[tokio::test]
async fn keyvalue_atomic_swap() {
    init_tracing_tests();

    const BUCKET: &str = "test_bucket";
    const KEY_1: &str = "test_key_1";
    const KEY_2: &str = "test_key_2";
    const VALUE: &[u8] = b"hello";
    const VALUE_AFTER_SWAP_1: &[u8] = b"cruel";
    const VALUE_AFTER_SWAP_2: &[u8] = b"world";

    let db_dir = tempfile::tempdir().unwrap();
    let db = RedbStorage::new(db_dir.path()).unwrap();
    let keyvalue_ctx = KeyValueCtx::new(db.clone(), "test".to_string());

    // Write a value to the key-value store
    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::Write {
            bucket: BUCKET.to_string(),
            key: KEY_1.to_string(),
            value: VALUE.to_vec(),
        },
    )
    .await;

    assert_eq!(resp, KvStoreResponse::Write,);

    // Swap it
    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::AtomicSwap {
            bucket: BUCKET.to_string(),
            key: KEY_1.to_string(),
            value: VALUE_AFTER_SWAP_1.to_vec(),
        },
    )
    .await;

    assert_eq!(resp, KvStoreResponse::AtomicSwap,);

    // Read it back
    let resp = execute_component::<KvStoreResponse>(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::Read {
            bucket: BUCKET.to_string(),
            key: KEY_1.to_string(),
        },
    )
    .await;

    assert_eq!(
        resp,
        KvStoreResponse::Read {
            value: VALUE_AFTER_SWAP_1.to_vec()
        },
    );

    // Swap it, without setting first
    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::AtomicSwap {
            bucket: BUCKET.to_string(),
            key: KEY_2.to_string(),
            value: VALUE_AFTER_SWAP_2.to_vec(),
        },
    )
    .await;

    assert_eq!(resp, KvStoreResponse::AtomicSwap,);

    // Read it back
    let resp = execute_component::<KvStoreResponse>(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx),
        KvStoreRequest::Read {
            bucket: BUCKET.to_string(),
            key: KEY_2.to_string(),
        },
    )
    .await;

    assert_eq!(
        resp,
        KvStoreResponse::Read {
            value: VALUE_AFTER_SWAP_2.to_vec()
        },
    );
}

#[tokio::test]
async fn keyvalue_batch() {
    init_tracing_tests();

    const BUCKET: &str = "test_bucket";
    let db_dir = tempfile::tempdir().unwrap();
    let db = RedbStorage::new(db_dir.path()).unwrap();
    let keyvalue_ctx = KeyValueCtx::new(db.clone(), "test".to_string());

    // Prepare batch data
    let mut values: HashMap<String, Vec<u8>> = HashMap::new();
    for i in 1..=10 {
        values.insert(format!("key_{i}"), format!("value_{i}").into_bytes());
    }

    // Perform batch write
    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::BatchWrite {
            bucket: BUCKET.to_string(),
            values: values.clone(),
        },
    )
    .await;

    assert_eq!(resp, KvStoreResponse::BatchWrite,);

    // Perform batch read
    let resp = execute_component::<KvStoreResponse>(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::BatchRead {
            bucket: BUCKET.to_string(),
            keys: values.keys().cloned().collect(),
        },
    )
    .await;

    match resp {
        KvStoreResponse::BatchRead {
            values: read_values,
        } => {
            // Verify all keys were read correctly
            for (key, expected_value) in &values {
                assert_eq!(read_values.get(key).unwrap(), expected_value);
            }
        }
        _ => panic!("Expected BatchRead response"),
    }

    // Delete a few keys
    let keys_to_delete: Vec<String> = vec![
        "key_1".to_string(),
        "key_5".to_string(),
        "key_7".to_string(),
    ];
    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::BatchDelete {
            bucket: BUCKET.to_string(),
            keys: keys_to_delete.clone(),
        },
    )
    .await;

    assert_eq!(resp, KvStoreResponse::BatchDelete,);

    // Verify deleted keys are no longer present
    let resp = execute_component::<KvStoreResponse>(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::BatchRead {
            bucket: BUCKET.to_string(),
            keys: values.keys().cloned().collect(),
        },
    )
    .await;

    match resp {
        KvStoreResponse::BatchRead {
            values: read_values,
        } => {
            // Verify deleted keys are no longer present
            for (key, expected_value) in &values {
                if keys_to_delete.contains(key) {
                    assert!(
                        !read_values.contains_key(key),
                        "Key {key} should have been deleted",
                    );
                } else {
                    // For keys that were not deleted, verify their values
                    assert_eq!(read_values.get(key).unwrap(), expected_value);
                }
            }
        }
        _ => panic!("Expected BatchRead response"),
    }

    // sanity check that batch operations work on the same keys as regular operations

    let resp = execute_component::<KvStoreResponse>(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::Read {
            bucket: BUCKET.to_string(),
            key: "key_2".to_string(),
        },
    )
    .await;

    assert_eq!(
        resp,
        KvStoreResponse::Read {
            value: values.get("key_2").unwrap().clone()
        }
    );

    let err = try_execute_component::<KvStoreResponse>(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx),
        KvStoreRequest::Read {
            bucket: BUCKET.to_string(),
            key: "key_5".to_string(),
        },
    )
    .await
    .unwrap_err();

    assert_eq!(
        err,
        KvStoreError::MissingKey {
            bucket: BUCKET.to_string(),
            key: "key_5".to_string(),
        }
        .to_string()
    );
}

#[tokio::test]
async fn keyvalue_list() {
    init_tracing_tests();

    const BUCKET: &str = "test_bucket";
    let db_dir = tempfile::tempdir().unwrap();
    let db = RedbStorage::new(db_dir.path()).unwrap();
    let mut keyvalue_ctx = KeyValueCtx::new(db.clone(), "test".to_string());

    // Prepare batch data
    let mut values: HashMap<String, Vec<u8>> = HashMap::new();
    for i in 1..=10 {
        values.insert(format!("key_{i}"), format!("foo_{i}").into_bytes());
        values.insert(format!("abc_{i}"), format!("bar_{i}").into_bytes());
    }

    // Perform batch write
    let _: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::BatchWrite {
            bucket: BUCKET.to_string(),
            values: values.clone(),
        },
    )
    .await;

    // list all keys
    let resp = execute_component::<KvStoreResponse>(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::ListKeys {
            bucket: BUCKET.to_string(),
            cursor: None,
        },
    )
    .await;

    let mut next_cursor = None;
    match resp {
        KvStoreResponse::ListKeys { keys, cursor } => {
            // Verify all keys are present
            for (i, key) in keys.iter().enumerate() {
                assert!(values.contains_key(key), "Key {key} should be present");
                // start the next cursor after 5 keys
                if i == 5 {
                    next_cursor = Some(key.clone());
                }
            }

            assert!(cursor.is_none(), "Cursor should be None");
        }
        _ => panic!("Expected ListKeys response"),
    }

    // list all keys with a cursor
    let resp = execute_component::<KvStoreResponse>(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::ListKeys {
            bucket: BUCKET.to_string(),
            cursor: next_cursor.clone(),
        },
    )
    .await;

    match resp {
        KvStoreResponse::ListKeys { keys, cursor: _ } => {
            // Verify all keys are present
            for key in keys.iter() {
                assert!(values.contains_key(key), "Key {key} should be present");
            }

            // should have only iterated after cursor
            assert_eq!(
                keys.len(),
                values.len() - 5,
                "Expected remaining keys after cursor"
            );
            assert_eq!(keys.first().unwrap(), next_cursor.as_ref().unwrap());
        }
        _ => panic!("Expected ListKeys response"),
    }

    // change page size
    keyvalue_ctx.page_size = Some(3);

    let resp = execute_component::<KvStoreResponse>(
        COMPONENT_KV_STORE_BYTES,
        Some(keyvalue_ctx.clone()),
        KvStoreRequest::ListKeys {
            bucket: BUCKET.to_string(),
            cursor: next_cursor.clone(),
        },
    )
    .await;

    match resp {
        KvStoreResponse::ListKeys { keys, cursor: _ } => {
            // Verify all keys are present
            for key in keys.iter() {
                assert!(values.contains_key(key), "Key {key} should be present");
            }

            // limited responses
            assert_eq!(keys.len(), 3, "Expected 3 keys per page");
            assert_eq!(keys.first().unwrap(), next_cursor.as_ref().unwrap());
        }
        _ => panic!("Expected ListKeys response"),
    }
}
