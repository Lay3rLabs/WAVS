mod helpers;

use crate::helpers::exec::execute_component;
use utils::{init_tracing_tests, test_utils::mock_engine::COMPONENT_SQUARE_BYTES};

use example_types::{SquareRequest, SquareResponse};

#[tokio::test]
async fn basic_execution() {
    init_tracing_tests();

    let resp: Vec<SquareResponse> = execute_component(
        COMPONENT_SQUARE_BYTES,
        Default::default(),
        None,
        SquareRequest::new(5),
    )
    .await;

    assert_eq!(resp[0].y, 25, "Expected output to be 25, got {}", resp[0].y);
}
