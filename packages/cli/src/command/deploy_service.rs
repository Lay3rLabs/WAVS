use anyhow::{Context, Result};
use layer_climb::prelude::ConfigAddressExt;
use rand::rngs::OsRng;
use std::{collections::HashMap, path::PathBuf};
use utils::{
    avs_client::AvsClientDeployer, filesystem::workspace_path, types::ChainName, ServiceID,
    WorkflowID,
};
use wavs::{apis::dispatcher::ServiceConfig, Digest};

use crate::{
    args::{CliSubmitKind, CliTriggerKind},
    clients::{
        example_cosmos_client::SimpleCosmosTriggerClient,
        example_eth_client::{example_submit::SimpleSubmit, SimpleEthTriggerClient},
        HttpClient,
    },
    context::CliContext,
    deploy::{ServiceInfo, ServiceSubmitInfo, ServiceTriggerInfo},
    util::read_component,
};

pub struct DeployService {
    pub service_id: ServiceID,
    pub workflows: HashMap<WorkflowID, ServiceInfo>,
}

pub struct DeployServiceArgs {
    pub register_operator: bool,
    pub component: ComponentSource,
    pub trigger: CliTriggerKind,
    pub trigger_event_name: Option<String>,
    pub trigger_chain: Option<ChainName>,
    pub trigger_address: Option<String>,
    pub submit_address: Option<String>,
    pub cosmos_trigger_code_id: Option<u64>,
    pub submit: CliSubmitKind,
    pub submit_chain: Option<ChainName>,
    pub service_config: Option<ServiceConfig>,
}

pub enum ComponentSource {
    Path(PathBuf),
    Digest(Digest),
}

impl DeployService {
    pub async fn run(
        ctx: &CliContext,
        DeployServiceArgs {
            register_operator,
            component,
            trigger,
            trigger_event_name,
            trigger_chain,
            trigger_address,
            submit_address,
            cosmos_trigger_code_id,
            submit,
            submit_chain,
            service_config,
        }: DeployServiceArgs,
    ) -> Result<Option<Self>> {
        let deployment = ctx.deployment.lock().unwrap().clone();

        let trigger_info: ServiceTriggerInfo = match trigger {
            CliTriggerKind::EthContractEvent => {
                let chain_name = trigger_chain.context("must have trigger chain for contract")?;
                let trigger_event_name =
                    trigger_event_name.context("must have trigger event name")?;

                let address = match trigger_address {
                    None => {
                        SimpleEthTriggerClient::deploy(
                            ctx.get_eth_client(&chain_name)?.eth.provider.clone(),
                        )
                        .await?
                    }
                    Some(address) => address.parse()?,
                };

                let mut event_hash: [u8; 32] = [0; 32];
                event_hash.copy_from_slice(&hex::decode(trigger_event_name)?);

                ServiceTriggerInfo::EthSimpleContract {
                    chain_name,
                    address: address.into(),
                    event_hash,
                }
            }
            CliTriggerKind::CosmosContractEvent => {
                let chain_name = trigger_chain.context("must have trigger chain for contract")?;
                let trigger_event_name =
                    trigger_event_name.context("must have trigger event name")?;

                let signing_client = ctx.get_cosmos_client(&chain_name)?;

                let address = match trigger_address {
                    None => {
                        let code_id = match cosmos_trigger_code_id {
                            Some(code_id) => code_id,
                            None => {
                                let path_to_wasm = workspace_path()
                                    .join("examples")
                                    .join("build")
                                    .join("contracts")
                                    .join("simple_example.wasm");

                                let wasm_byte_code = std::fs::read(path_to_wasm)?;

                                let (code_id, _) = signing_client
                                    .contract_upload_file(wasm_byte_code, None)
                                    .await?;

                                code_id
                            }
                        };

                        SimpleCosmosTriggerClient::new_code_id(signing_client, code_id)
                            .await?
                            .contract_address
                    }
                    Some(address) => signing_client
                        .querier
                        .chain_config
                        .parse_address(&address)?,
                };

                ServiceTriggerInfo::CosmosSimpleContract {
                    chain_name,
                    address,
                    event_type: trigger_event_name,
                }
            }
        };

        let submit_info: ServiceSubmitInfo = match submit {
            CliSubmitKind::SimpleEthContract => {
                let chain_name = submit_chain.expect("must have submit chain for contract");

                let core_contracts = match deployment.eigen_core.get(&chain_name) {
                    Some(core_contracts) => core_contracts.clone(),
                    None => {
                        tracing::error!(
                                "Eigenlayer core contracts not deployed for chain {}, deploy those first!",
                                chain_name
                            );
                        return Ok(None);
                    }
                };

                let eth_client = ctx.get_eth_client(&chain_name)?;

                tracing::info!("deploying eth submit contract on eigenlayer");

                let deployer =
                    AvsClientDeployer::new(eth_client.eth).core_addresses(core_contracts);

                let avs_client = match submit_address {
                    Some(submit_address) => deployer.into_client(submit_address.parse()?).await?,
                    None => {
                        let simple_submit =
                            SimpleSubmit::deploy(deployer.eth.provider.clone()).await?;

                        deployer
                            .deploy_service_manager(*simple_submit.address(), None)
                            .await?
                    }
                };

                if register_operator {
                    avs_client.register_operator(&mut OsRng).await?;
                }

                ServiceSubmitInfo::EigenLayer {
                    chain_name,
                    service_manager_address: avs_client.service_manager,
                }
            }
        };

        let service_info = ServiceInfo {
            trigger: trigger_info,
            submit: submit_info,
        };

        let http_client = HttpClient::new(&ctx.config);

        let digest = match component {
            ComponentSource::Path(path) => {
                let wasm_bytes = read_component(path)?;
                http_client.upload_component(wasm_bytes).await?
            }
            ComponentSource::Digest(digest) => digest,
        };

        let (service_id, workflow_id) = http_client
            .create_service(
                service_info.clone(),
                digest,
                service_config.unwrap_or_default(),
            )
            .await?;

        let mut workflows = HashMap::new();
        workflows.insert(workflow_id.clone(), service_info.clone());

        let mut deployment = deployment;
        deployment
            .services
            .insert(service_id.clone(), workflows.clone());

        *ctx.deployment.lock().unwrap() = deployment;

        Ok(Some(Self {
            service_id,
            workflows,
        }))
    }
}
