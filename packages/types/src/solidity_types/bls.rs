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
}
