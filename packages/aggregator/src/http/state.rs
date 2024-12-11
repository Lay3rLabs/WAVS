use std::{
    collections::HashMap,
    iter,
    sync::{Arc, RwLock},
};

use alloy::{
    contract::{ContractInstance, Interface},
    dyn_abi::{DynSolValue, JsonAbiExt},
    json_abi::JsonAbi,
    primitives::{eip191_hash_message, keccak256, Address, FixedBytes, TxHash, U256},
    sol_types::SolCall,
};
use anyhow::{ensure, Context};
use utils::{
    eigen_client::solidity_types::HttpSigningProvider,
    eth_client::{AddTaskRequest, OperatorSignature},
};

use crate::{
    config::Config,
    solidity_types::erc1271::IERC1271::{self, IERC1271Instance},
};

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
    pub aggregator_state: Arc<RwLock<HashMap<String, Task>>>,
}

impl HttpState {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        Ok(Self {
            config,
            aggregator_state: Default::default(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Task {
    pub signatures: HashMap<Address, Vec<u8>>,
    pub operators: Vec<Address>,
    pub service: Address,
    pub reference_block: u64,
    pub function: alloy::json_abi::Function,
    /// Function input without a signature
    pub input: Vec<u8>,
    pub erc1271: Address,
}

impl From<AddTaskRequest> for Task {
    fn from(value: AddTaskRequest) -> Self {
        Self {
            signatures: Default::default(),
            operators: value.operators,
            service: value.service,
            reference_block: value.reference_block,
            function: value.function,
            input: value.input,
            erc1271: value.erc1271,
        }
    }
}

impl Task {
    pub fn add_signature(&mut self, signature: OperatorSignature) -> anyhow::Result<()> {
        let OperatorSignature { address, signature } = signature;
        ensure!(
            self.operators.contains(&address),
            "Cannot sign not as an operator"
        );
        self.signatures.insert(address, signature);

        Ok(())
    }

    /// Try to complete task
    ///
    /// If enough signatures returns TxHash of completed task
    /// If not enough signatures returns `None`
    /// Otherwise returns an error
    pub async fn try_completing(
        &self,
        task_name: &str,
        provider: &HttpSigningProvider,
    ) -> anyhow::Result<Option<TxHash>> {
        let erc1271 = IERC1271Instance::new(self.erc1271, provider);
        let hash = eip191_hash_message(keccak256(task_name));
        let signature_bytes = signature_bytes(self.signatures.clone(), self.reference_block);

        // Check if we have enough weight
        match erc1271
            .isValidSignature(hash, signature_bytes.clone().into())
            .call()
            .await
        {
            Ok(valid_signature) => {
                if valid_signature.magicValue != IERC1271::isValidSignatureCall::SELECTOR {
                    // Check magic return value, to ensure we didn't hit non-compatible with ERC1271 contract
                    return Err(anyhow::anyhow!(
                        "Unexpected isValidSignature return value:{} expected:{}",
                        valid_signature.magicValue,
                        FixedBytes::from(IERC1271::isValidSignatureCall::SELECTOR)
                    ));
                }
                if valid_signature.magicValue == IERC1271::isValidSignatureCall::SELECTOR {
                    tracing::info!("Got enough signatures, submitting tx");
                    let avs_contract = ContractInstance::new(
                        self.service,
                        provider,
                        Interface::new(JsonAbi::from_iter(iter::once(
                            self.function.clone().into(),
                        ))),
                    );

                    // Searching signature param index
                    let signature_index = self
                        .function
                        .inputs
                        .iter()
                        .enumerate()
                        .find_map(|(idx, param)| param.name.eq("signature").then_some(idx))
                        .context("signature")?;
                    let mut args = self.function.abi_decode_input(&self.input, false)?;
                    let DynSolValue::Bytes(bytes) = &mut args[signature_index] else {
                        return Err(anyhow::anyhow!("Signature supposed to be bytes"));
                    };
                    *bytes = signature_bytes;

                    let receipt = avs_contract
                        .function(&self.function.name, &args)?
                        .gas(500000)
                        .send()
                        .await?
                        .get_receipt()
                        .await?;
                    // One operator is enough for submission, no need to store task
                    return match receipt.status() {
                        true => Ok(Some(receipt.transaction_hash)),
                        false => Err(anyhow::anyhow!("Failed to submit task")),
                    };
                }
            }
            // TODO: Figure out if we have correct signature based on error
            // We should only pass on `InsufficientWeight()`
            Err(e) => {
                tracing::error!("Signature check failed {e:?}");
            }
        };
        Ok(None)
    }
}

fn signature_bytes(signatures: HashMap<Address, Vec<u8>>, reference_block: u64) -> Vec<u8> {
    let (operators, signatures): (Vec<_>, Vec<_>) = signatures
        .into_iter()
        .map(|(operator, signature)| {
            (
                DynSolValue::Address(operator),
                DynSolValue::Bytes(signature),
            )
        })
        .unzip();
    DynSolValue::Tuple(vec![
        DynSolValue::Array(operators),
        DynSolValue::Array(signatures),
        DynSolValue::Uint(U256::from(reference_block), 32),
    ])
    .abi_encode_params()
}
