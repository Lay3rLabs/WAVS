use alloy_rpc_types_eth::TransactionReceipt;
use alloy_sol_types::SolEvent;

pub trait SolidityEventFinder<E> {
    fn solidity_event(&self) -> Option<E>;
}

impl<E: SolEvent> SolidityEventFinder<E> for TransactionReceipt {
    fn solidity_event(&self) -> Option<E> {
        self.inner
            .logs()
            .iter()
            .find_map(|log| log.log_decode::<E>().map(|log| log.inner.data).ok())
    }
}
