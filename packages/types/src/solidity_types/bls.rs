// BLS12-381 Solidity ABI bindings
// These are separate from the secp256k1 bindings because the IWavsServiceHandler
// interfaces have different SignatureData structs:
// - secp256k1: signers (address[]), signatures (bytes[]), referenceBlock (uint32)
// - BLS: signerPubkeys (bytes[]), aggregateSignature (bytes), referenceBlock (uint32)

mod bls_service_handler {
    alloy_sol_macro::sol!(
        #[allow(missing_docs)]
        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
        IWavsServiceHandler,
        "./src/contracts/solidity/abi/bls/IWavsServiceHandler.json"
    );
}

mod bls_stake_registry {
    alloy_sol_macro::sol!(
        #[allow(missing_docs)]
        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
        IPOAStakeRegistry,
        "./src/contracts/solidity/abi/bls/IPOAStakeRegistry.json"
    );
}

mod bls_service_manager {
    alloy_sol_macro::sol!(
        #[allow(missing_docs)]
        #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
        IWavsServiceManager,
        "./src/contracts/solidity/abi/bls/IWavsServiceManager.json"
    );
}

// Re-export with namespaced paths to avoid collision with secp256k1 bindings
pub use bls_service_handler::IWavsServiceHandler as BlsServiceHandler;
pub use bls_service_manager::IWavsServiceManager as BlsServiceManager;
pub use bls_stake_registry::IPOAStakeRegistry as BlsStakeRegistry;

// Feature-gated RPC bindings for on-chain interaction (contract calls).
// These mirror the pattern in rpc.rs but for BLS-specific contract interfaces.
cfg_if::cfg_if! {
    if #[cfg(feature = "solidity-rpc")] {
        mod bls_service_handler_rpc {
            alloy_sol_macro::sol!(
                #[allow(missing_docs)]
                #[sol(rpc)]
                #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
                IWavsServiceHandler,
                "./src/contracts/solidity/abi/bls/IWavsServiceHandler.json"
            );
        }

        mod bls_service_manager_rpc {
            alloy_sol_macro::sol!(
                #[allow(missing_docs)]
                #[sol(rpc)]
                #[derive(Debug)]
                IWavsServiceManager,
                "./src/contracts/solidity/abi/bls/IWavsServiceManager.json"
            );
        }

        pub use bls_service_handler_rpc::IWavsServiceHandler as BlsServiceHandlerRpc;
        pub use bls_service_manager_rpc::IWavsServiceManager as BlsServiceManagerRpc;
        // Re-export BLS service manager's view of handler types (for validate() calls)
        // Same pattern as rpc.rs ServiceManagerEnvelope/ServiceManagerSignatureData
        pub use bls_service_manager_rpc::IWavsServiceHandler::Envelope as BlsServiceManagerEnvelope;
        pub use bls_service_manager_rpc::IWavsServiceHandler::SignatureData as BlsServiceManagerSignatureData;

        pub type BlsServiceHandlerInstance = BlsServiceHandlerRpc::IWavsServiceHandlerInstance<alloy_provider::DynProvider>;
        pub type BlsServiceManagerInstance = BlsServiceManagerRpc::IWavsServiceManagerInstance<alloy_provider::DynProvider>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Bytes;

    #[test]
    fn bls_service_handler_signature_data_fields() {
        // Verify the BLS SignatureData struct has the expected fields
        let sig_data = BlsServiceHandler::SignatureData {
            signerPubkeys: vec![Bytes::from(vec![0u8; 128])],
            aggregateSignature: Bytes::from(vec![0u8; 256]),
            referenceBlock: 42,
        };
        assert_eq!(sig_data.signerPubkeys.len(), 1);
        assert_eq!(sig_data.aggregateSignature.len(), 256);
        assert_eq!(sig_data.referenceBlock, 42);
    }

    #[test]
    fn bls_bindings_compile() {
        // Just verify the types are accessible -- compilation is the test
        let _handler_type = std::any::type_name::<BlsServiceHandler::SignatureData>();
        let _registry_type = std::any::type_name::<BlsStakeRegistry::IPOAStakeRegistryCalls>();
        let _manager_type = std::any::type_name::<BlsServiceManager::IWavsServiceManagerCalls>();
    }

    #[cfg(feature = "solidity-rpc")]
    #[test]
    fn bls_rpc_bindings_compile() {
        // Verify the RPC types are accessible when solidity-rpc feature is enabled.
        let _handler_type = std::any::type_name::<BlsServiceHandlerInstance>();
        let _manager_type = std::any::type_name::<BlsServiceManagerInstance>();
    }
}
