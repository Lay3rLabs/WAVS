mod helpers;

use crate::helpers::exec::execute_component;
use example_types::{KvStoreRequest, KvStoreResponse};
use utils::{init_tracing_tests, test_utils::mock_engine::COMPONENT_KV_STORE_BYTES};

#[tokio::test]
async fn keyvalue_execution() {
    init_tracing_tests();

    let resp: KvStoreResponse = execute_component(
        COMPONENT_KV_STORE_BYTES,
        KvStoreRequest::Write {
            key: "hello".to_string(),
            value: b"world".to_vec(),
            read_immediately: true,
        },
    )
    .await;

    match resp {
        KvStoreResponse::Read { value } => {
            assert_eq!(
                value,
                b"world".to_vec(),
                "Expected value to be 'world', got {value:?}"
            );
        }
        _ => panic!("Expected Read response, got {resp:?}"),
    }
}
