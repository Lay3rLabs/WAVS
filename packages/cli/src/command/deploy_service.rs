use std::collections::BTreeMap;

use anyhow::{Context, Result};
use layer_climb::prelude::ConfigAddressExt;
use rand::rngs::OsRng;
use utils::{
    avs_client::AvsClientDeployer,
    filesystem::workspace_path,
    types::{ChainName, ComponentSource, ServiceConfig, Submit, Trigger, Workflow},
    ServiceID, WorkflowID,
};

use crate::{
    args::{CliSubmitKind, CliTriggerKind},
    clients::{
        example_cosmos_client::SimpleCosmosTriggerClient,
        example_eth_client::{
            example_submit::SimpleSubmit, SimpleEthSubmitClient, SimpleEthTriggerClient,
        },
        HttpClient,
    },
    command::deploy_eigen_service_manager::{
        DeployEigenServiceManager, DeployEigenServiceManagerArgs,
    },
    context::CliContext,
    deploy::CommandDeployResult,
};

pub struct DeployService {
    pub service_id: ServiceID,
    pub workflows: BTreeMap<WorkflowID, Workflow>,
}

impl std::fmt::Display for DeployService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DeployService")
    }
}

impl CommandDeployResult for DeployService {
    fn update_deployment(&self, _deployment: &mut crate::deploy::Deployment) {}
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

        let trigger: Trigger = match trigger {
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

                Trigger::EthContractEvent {
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

                Trigger::CosmosContractEvent {
                    chain_name,
                    address,
                    event_type: trigger_event_name,
                }
            }
        };

        let submit: Submit = match submit {
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

                let service_manager_address = match submit_address {
                    Some(submit_address) => {
                        // already have a submit address, but maybe we still want to register as an operator
                        if register_operator {
                            let deployer = AvsClientDeployer::new(eth_client.eth)
                                .core_addresses(core_contracts);

                            let avs_client = deployer.into_client(submit_address.parse()?).await?;

                            avs_client.register_operator(&mut OsRng).await?;
                        }

                        submit_address.parse()?
                    }
                    None => {
                        // fresh deployment, using "SimpleSubmit" handler
                        let simple_submit =
                            SimpleSubmit::deploy(eth_client.eth.provider.clone()).await?;

                        // re-use the same code that we use to deploy the service manager explicitly
                        let res = DeployEigenServiceManager::run(
                            ctx,
                            DeployEigenServiceManagerArgs {
                                chain: chain_name.clone(),
                                service_handler: *simple_submit.address(),
                                register_operator,
                            },
                        )
                        .await
                        .unwrap();

                        let DeployEigenServiceManager { address } = res;
                        // but for our "simple submit", we want to set the serviceManager for its custom security rules
                        let simple_submit_client = SimpleEthSubmitClient::new(
                            eth_client.eth.clone(),
                            *simple_submit.address(),
                        );
                        simple_submit_client
                            .set_service_manager_address(address)
                            .await?;

                        address
                    }
                };

                Submit::EigenContract {
                    chain_name,
                    service_manager: service_manager_address,
                    max_gas: None,
                }
            }
        };

        let http_client = HttpClient::new(&ctx.config);

        let digest = match component {
            ComponentSource::Bytecode(bytes) => http_client.upload_component(bytes).await?,
            ComponentSource::Download { url: _, digest } => digest,
            ComponentSource::Registry {
                registry: _,
                digest,
            } => digest,
            ComponentSource::Digest(digest) => digest,
        };

        let service_config = service_config.unwrap_or_default();

        let (service_id, workflow_id) = http_client
            .create_service(
                trigger.clone(),
                submit.clone(),
                digest,
                service_config.clone(),
            )
            .await?;

        let mut workflows = BTreeMap::new();
        workflows.insert(
            workflow_id.clone(),
            Workflow {
                trigger,
                component: service_config.component_id,
                submit,
            },
        );

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
