use anyhow::Result;
use cosmwasm_std::{Empty, Uint64};
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct SimpleCosmosTriggerClient {
    pub signing_client: SigningClient,
    pub contract_address: Address,
}

type TriggerId = Uint64;

pub use simple_example_cosmos::event::NewMessageEvent;

impl SimpleCosmosTriggerClient {
    pub fn new(signing_client: SigningClient, contract_address: Address) -> Self {
        Self {
            signing_client,
            contract_address,
        }
    }

    pub async fn new_code_id(signing_client: SigningClient, code_id: u64) -> Result<Self> {
        let (addr, _) = signing_client
            .contract_instantiate(None, code_id, "simple-trigger", &Empty {}, Vec::new(), None)
            .await?;

        Ok(Self::new(signing_client, addr))
    }

    pub async fn add_trigger(&self, data: Vec<u8>) -> Result<TriggerId> {
        // The execute message and event are from `examples/contracts/cosmwasm/simple`
        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub enum ExecuteMsg {
            // Proprietary per-app... but will emit an event registered with layer
            AddTrigger { data: Vec<u8> },
        }

        let res = self
            .signing_client
            .contract_execute(
                &self.contract_address,
                &ExecuteMsg::AddTrigger { data },
                Vec::new(),
                None,
            )
            .await?;

        CosmosTxEvents::from(&res)
            .attr_first("new-message", "id")
            .map_err(|_| anyhow::anyhow!("missing trigger id"))
            .and_then(|attr| {
                Uint64::try_from(attr.value()).map_err(|_| anyhow::anyhow!("invalid trigger id"))
            })
    }
}
