mod helpers;

use crate::helpers::exec::{execute_component, try_execute_component};
use example_types::{KvStoreError, KvStoreRequest, KvStoreResponse};
use utils::{init_tracing_tests, test_utils::mock_engine::COMPONENT_KV_STORE_BYTES};

#[tokio::test]
async fn keyvalue_execution() {
    init_tracing_tests();

    const KEY: &'static str = "test_key";
    const BAD_KEY: &'static str = "bad_test_key";
    const VALUE: &[u8] = b"hello";

    // Write a value to the key-value store
    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
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

    // now read the right key

    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
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
