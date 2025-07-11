mod helpers;

use crate::helpers::exec::{execute_component, try_execute_component};
use example_types::{KvStoreError, KvStoreRequest, KvStoreResponse};
use utils::{
    init_tracing_tests, storage::db::RedbStorage, test_utils::mock_engine::COMPONENT_KV_STORE_BYTES,
};
use wavs_engine::context::KeyValueCtx;

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
