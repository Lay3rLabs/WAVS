use anyhow::Result;

use cw_orch::daemon::{DaemonAsync, DaemonAsyncBuilder};
use cw_orch::environment::{ChainInfoOwned, ChainKind, NetworkInfo};
use cw_orch::prelude::Addr;
use lavs_apis::id::TaskId;
use lavs_apis::tasks::{CustomQueryMsg, ListOpenResponse, OpenTaskOverview, QueryMsg};
use lavs_apis::verifier_simple::{
    ExecuteMsg, OperatorVoteInfoResponse, QueryMsg as VerifierQueryMsg,
};
use layer_climb::prelude::*;

pub const SLAY3R_NETWORK: NetworkInfo = NetworkInfo {
    chain_name: "slay3r",
    pub_address_prefix: "slay3r",
    coin_type: 118u32,
};

pub fn daemon_builder(
    kind: ChainKind,
    grpc_url: String,
    chain_id: String,
    gas_denom: String,
    gas_price: f64,
) -> DaemonAsyncBuilder {
    let mut builder = DaemonAsyncBuilder::default();
    let chain_info = ChainInfoOwned {
        chain_id,
        gas_denom,
        gas_price,
        grpc_urls: vec![grpc_url],
        lcd_url: None,
        fcd_url: None,
        network_info: SLAY3R_NETWORK.into(),
        kind,
    };
    builder.chain(chain_info);
    builder
}

#[derive(Clone)]
pub struct AppData {
    pub task_queue_addr: Addr,
    pub verifier_addr: Addr,
    pub lay3r: DaemonAsync,
    pub query_client: QueryClient,
}

impl AppData {
    pub async fn get_tasks(&self) -> anyhow::Result<Vec<OpenTaskOverview>> {
        let query: QueryMsg = CustomQueryMsg::ListOpen {}.into();
        let raw = self.query_client.contract_smart_raw(&self.query_client.chain_config.parse_address(self.task_queue_addr.as_str())?, &query).await?; 
        let raw_str = std::str::from_utf8(&raw)?;
        eprintln!("raw decoded: {}", raw_str);
        let res: ListOpenResponse = self.query_client.contract_smart(&self.query_client.chain_config.parse_address(self.task_queue_addr.as_str())?, &query).await?; 
        eprintln!("got response!");
        //let res: ListOpenResponse = self.lay3r.query(&query, &self.task_queue_addr).await?;
        let operator = self.lay3r.sender().into_string();
        let mut tasks = Vec::with_capacity(res.tasks.len());
        for t in res.tasks {
            let query: VerifierQueryMsg = VerifierQueryMsg::OperatorVote {
                task_contract: self.task_queue_addr.to_string(),
                task_id: t.id,
                operator: operator.clone(),
            };
            let res: Option<OperatorVoteInfoResponse> =
                self.lay3r.query(&query, &self.verifier_addr).await?;
            if res.is_none() {
                tasks.push(t);
            }
        }
        Ok(tasks)
    }

    pub async fn submit_result(&self, task_id: TaskId, result: String) -> Result<()> {
        let msg = ExecuteMsg::ExecutedTask {
            task_queue_contract: self.task_queue_addr.to_string(),
            task_id,
            result,
        };
        self.lay3r.execute(&msg, &[], &self.verifier_addr).await?;
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct QueueExecutor {
    pub builder: DaemonAsyncBuilder,
}

impl QueueExecutor {
    pub fn new(
        kind: Option<ChainKind>,
        grpc_url: Option<String>,
        chain_id: Option<String>,
        gas_denom: Option<String>,
        gas_price: Option<f64>,
    ) -> Self {
        let builder = daemon_builder(
            kind.unwrap_or(ChainKind::Local),
            grpc_url.unwrap_or("http://localhost:9090".to_string()),
            chain_id.unwrap_or("slay3r-local".to_string()),
            gas_denom.unwrap_or("uslay".to_string()),
            gas_price.unwrap_or(0.025),
        );

        Self { builder }
    }
}
