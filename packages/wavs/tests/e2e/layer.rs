use anyhow::{Context, Result};
use lavs_apis::{id::TaskId, tasks as task_queue};
use layer_climb::{prelude::*, proto::abci::TxResponse, signing::SigningClient};
use serde::Serialize;
use wavs::config::Config;

#[allow(dead_code)]
pub struct LayerTestApp {
    pub layer_client: SigningClient,
    pub task_queue: LayerTaskQueueContract,
}

impl LayerTestApp {
    pub async fn new(config: Config) -> Self {
        // get all env vars
        let seed_phrase =
            std::env::var("WAVS_E2E_LAYER_MNEMONIC").expect("WAVS_E2E_LAYER_MNEMONIC not set");
        let task_queue_addr = std::env::var("WAVS_E2E_LAYER_TASK_QUEUE_ADDRESS")
            .expect("WAVS_E2E_LAYER_TASK_QUEUE_ADDRESS not set");

        let chain_config: ChainConfig = config.layer_chain_config().unwrap().into();

        let key_signer = KeySigner::new_mnemonic_str(&seed_phrase, None).unwrap();
        let signing_client = SigningClient::new(chain_config.clone(), key_signer)
            .await
            .unwrap();

        tracing::info!(
            "Creating service on task queue contract: {}",
            task_queue_addr
        );
        let task_queue_addr = chain_config.parse_address(&task_queue_addr).unwrap();

        let task_queue = LayerTaskQueueContract::new(signing_client.clone(), task_queue_addr)
            .await
            .unwrap();

        Self {
            layer_client: signing_client,
            task_queue,
        }
    }
}

pub struct LayerTaskQueueContract {
    pub client: SigningClient,
    pub addr: Address,
    pub _verifier: LayerVerifierContract,
    pub task_cost: Option<Coin>,
}

impl LayerTaskQueueContract {
    pub async fn new(client: SigningClient, addr: Address) -> Result<Self> {
        let resp: task_queue::ConfigResponse = client
            .querier
            .contract_smart(
                &addr,
                &task_queue::QueryMsg::Custom(task_queue::CustomQueryMsg::Config {}),
            )
            .await?;

        let task_cost = match resp.requestor {
            task_queue::Requestor::Fixed(_) => None,
            task_queue::Requestor::OpenPayment(coin) => Some(new_coin(coin.amount, coin.denom)),
        };

        let verifier = LayerVerifierContract::new(
            client.clone(),
            client.querier.chain_config.parse_address(&resp.verifier)?,
        )
        .await?;

        Ok(Self {
            client,
            addr,
            _verifier: verifier,
            task_cost,
        })
    }

    pub async fn submit_task(
        &self,
        description: impl ToString,
        payload: impl Serialize,
    ) -> Result<TxResponse> {
        let msg = task_queue::ExecuteMsg::Custom(task_queue::CustomExecuteMsg::Create {
            description: description.to_string(),
            timeout: None,
            payload: serde_json::to_value(payload)?,
            with_completed_hooks: None,
            with_timeout_hooks: None,
        });

        let funds = match self.task_cost.as_ref() {
            Some(cost) => vec![cost.clone()],
            None => vec![],
        };

        self.client
            .contract_execute(&self.addr, &msg, funds, None)
            .await
            .context("submit task")
    }

    pub async fn query_task(&self, id: TaskId) -> Result<task_queue::TaskResponse> {
        self.client
            .querier
            .contract_smart(
                &self.addr,
                &task_queue::QueryMsg::Custom(task_queue::CustomQueryMsg::Task { id }),
            )
            .await
            .context("query task")
    }
}

pub struct LayerVerifierContract {
    pub _client: SigningClient,
    pub _addr: Address,
}

impl LayerVerifierContract {
    pub async fn new(client: SigningClient, addr: Address) -> Result<Self> {
        Ok(Self {
            _client: client,
            _addr: addr,
        })
    }
}
