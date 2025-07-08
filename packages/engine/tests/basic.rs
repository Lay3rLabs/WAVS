mod helpers;

use crate::helpers::exec::execute_component;
use utils::{
    init_tracing_tests,
    test_utils::mock_engine::{SquareIn, SquareOut, COMPONENT_SQUARE},
};

#[tokio::test]
async fn basic_execution() {
    init_tracing_tests();

    let resp: SquareOut = execute_component(COMPONENT_SQUARE, SquareIn::new(5)).await;

    assert_eq!(resp.y, 25, "Expected output to be 25, got {}", resp.y);
}
