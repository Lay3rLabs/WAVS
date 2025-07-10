mod helpers;

use crate::helpers::exec::{execute_component, try_execute_component};
use example_types::{KvStoreError, KvStoreRequest, KvStoreResponse};
use utils::{
    init_tracing_tests, storage::db::RedbStorage, test_utils::mock_engine::COMPONENT_KV_STORE_BYTES,
};
use wavs_engine::KeyValueCtx;

#[tokio::test]
async fn keyvalue_basic() {
    init_tracing_tests();

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
            key: KEY.to_string(),
        },
    )
    .await
    .unwrap_err();

    assert_eq!(
        err,
        KvStoreError::MissingKey {
            key: KEY.to_string(),
        }
        .to_string()
    );
}

#[tokio::test]
async fn keyvalue_wrong_key() {
    init_tracing_tests();

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
            key: BAD_KEY.to_string(),
        },
    )
    .await
    .unwrap_err();

    assert_eq!(
        err,
        KvStoreError::MissingKey {
            key: BAD_KEY.to_string(),
        }
        .to_string()
    );
}
