use anyhow::Result;

use cw_orch::daemon::{DaemonAsync, DaemonAsyncBuilder};
use cw_orch::environment::{ChainInfoOwned, ChainKind, NetworkInfo};
use cw_orch::prelude::{Addr, ChainInfo};
use lch_apis::tasks::{CustomQueryMsg, ListOpenResponse, OpenTaskOverview, QueryMsg};
use lch_apis::verifier_simple::{
    ExecuteMsg, OperatorVoteInfoResponse, QueryMsg as VerifierQueryMsg,
};

pub const SLAY3R_NETWORK: NetworkInfo = NetworkInfo {
    chain_name: "slay3r",
    pub_address_prefix: "slay3r",
    coin_type: 118u32,
};

pub const SLAY3R_LOCAL: ChainInfo = ChainInfo {
    chain_id: "slay3r-local",
    gas_denom: "uslay",
    gas_price: 0.025,
    grpc_urls: &["http://localhost:9090"],
    lcd_url: None,
    fcd_url: None,
    network_info: SLAY3R_NETWORK,
    kind: ChainKind::Local,
};

pub const SLAY3R_DEV: ChainInfo = ChainInfo {
    chain_id: "slay3r-dev",
    gas_denom: "uslay",
    gas_price: 0.025,
    grpc_urls: &["https://grpc.dev-cav3.net"],
    lcd_url: None,
    fcd_url: None,
    network_info: SLAY3R_NETWORK,
    kind: ChainKind::Testnet,
};

pub fn chain_info(
    kind: ChainKind,
    grpc_url: Option<String>,
    chain_id: Option<String>,
) -> ChainInfoOwned {
    let mut base: ChainInfoOwned = match kind {
        ChainKind::Local => SLAY3R_LOCAL,
        ChainKind::Testnet => SLAY3R_DEV,
        ChainKind::Mainnet => panic!("Mainnet not supported"),
    }
    .into();
    if let Some(grpc) = grpc_url {
        base.grpc_urls = vec![grpc];
    }
    if let Some(chain_id) = chain_id {
        base.chain_id = chain_id;
    }
    base
}

pub fn daemon_builder(
    kind: ChainKind,
    grpc_url: Option<String>,
    chain_id: Option<String>,
) -> DaemonAsyncBuilder {
    let chain_info = chain_info(kind, grpc_url, chain_id);
    let mut builder = DaemonAsyncBuilder::default();
    builder.chain(chain_info);
    builder
}

#[derive(derivative::Derivative)]
#[derivative(Default)]
struct QueueingMetadata {
    /// "local", "testnet", "mainnet" provides default settings
    #[derivative(Default(value = "ChainKind::Local"))]
    pub chain_kind: ChainKind,

    /// Override default: location of the gRPC server for lay3r chain
    pub grpc_url: Option<String>,
    /// Override default: chain id of the lay3r chain
    pub chain_id: Option<String>,
}

#[derive(Clone)]
pub struct AppData {
    pub task_queue_addr: Addr,
    pub verifier_addr: Addr,
    pub lay3r: DaemonAsync,
}

impl AppData {
    pub async fn get_tasks(&self) -> anyhow::Result<Vec<OpenTaskOverview>> {
        let query: QueryMsg = CustomQueryMsg::ListOpen {}.into();
        let res: ListOpenResponse = self.lay3r.query(&query, &self.task_queue_addr).await?;
        let operator = self.lay3r.sender().into_string();
        let mut tasks: Vec<OpenTaskOverview> = vec![];
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

    pub async fn submit_result(&self, task_id: u64, result: String) -> Result<()> {
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
    pub fn new() -> Self {
        let QueueingMetadata {
            chain_kind: kind,
            grpc_url,
            chain_id,
        } = QueueingMetadata::default();
        let builder = daemon_builder(
            kind,
            grpc_url.or(Some("http://localhost:9090".to_string())),
            chain_id,
        );

        Self { builder }
    }
}
