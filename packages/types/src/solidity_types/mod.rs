cfg_if::cfg_if! {
    if #[cfg(feature = "solidity-rpc")] {
        mod rpc;
        pub use rpc::*;
    } else {
        mod not_rpc;
        pub use not_rpc::*;
    }
}
