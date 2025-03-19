use std::num::NonZero;

use anyhow::{bail, Context, Result};
use layer_climb::prelude::ConfigAddressExt;
use wavs_types::{ByteArray, ChainName, ComponentSource, Service, ServiceConfig, Submit, Trigger};

use crate::{
    args::{CliSubmitKind, CliTriggerKind},
    clients::HttpClient,
    context::CliContext,
    deploy::CommandDeployResult,
};
use alloy_json_abi::Event;

pub struct DeployService {
    pub args: DeployServiceArgs,
    pub service: Service,
}

impl std::fmt::Display for DeployService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "New Service deployed to wavs")?;
        write!(f, "\n\n{:#?}", self.service)
    }
}

impl CommandDeployResult for DeployService {
    fn update_deployment(&self, deployment: &mut crate::deploy::Deployment) {
        deployment
            .services
            .insert(self.service.id.clone(), self.service.clone());
    }
}

#[derive(Clone)]
pub struct DeployServiceArgs {
    pub component: ComponentSource,
    pub trigger: CliTriggerKind,
    pub trigger_event_name: Option<String>,
    pub trigger_chain: Option<ChainName>,
    pub trigger_address: Option<String>,
    pub submit_address: Option<String>,
    pub submit: CliSubmitKind,
    pub submit_chain: Option<ChainName>,
    pub service_config: Option<ServiceConfig>,
}

impl DeployService {
    pub async fn run(ctx: &CliContext, args: DeployServiceArgs) -> Result<Option<Self>> {
        let DeployServiceArgs {
            component,
            trigger,
            trigger_event_name,
            trigger_chain,
            trigger_address,
            submit_address,
            submit,
            submit_chain,
            service_config,
        } = args.clone();

        let trigger: Trigger = match trigger {
            CliTriggerKind::EthContractEvent => {
                let chain_name = trigger_chain.context("must have trigger chain for contract")?;
                let address = trigger_address
                    .context("must have trigger address")?
                    .parse()?;

                // Order the match cases from most explicit to event parsing:
                // 1. 0x-prefixed hex string
                // 2. raw hex string (no 0x)
                // 3. event name to be parsed into signature
                let trigger_event_name = match trigger_event_name {
                    Some(name) if name.starts_with("0x") => name,
                    Some(name) if const_hex::const_check(name.as_bytes()).is_ok() => name,
                    Some(name) => Event::parse(&name)
                        .context("Invalid event signature format")?
                        .selector()
                        .to_string(),
                    None => bail!("Missing event trigger (requires hex signature or event name)"),
                };

                let mut event_hash: [u8; 32] = [0; 32];
                event_hash.copy_from_slice(&const_hex::decode(trigger_event_name)?);

                Trigger::EthContractEvent {
                    chain_name,
                    address,
                    event_hash: ByteArray::new(event_hash),
                }
            }
            CliTriggerKind::CosmosContractEvent => {
                let chain_name = trigger_chain.context("must have trigger chain for contract")?;
                let trigger_event_name =
                    trigger_event_name.context("must have trigger event name")?;
                let address = trigger_address.context("must have trigger address")?;

                let signing_client = ctx.get_cosmos_client(&chain_name)?;

                let address = signing_client
                    .querier
                    .chain_config
                    .parse_address(&address)?;

                Trigger::CosmosContractEvent {
                    chain_name,
                    address,
                    event_type: trigger_event_name,
                }
            }
            CliTriggerKind::EthBlockInterval => {
                let chain_name = trigger_chain.context("must have trigger chain for contract")?;

                Trigger::BlockInterval {
                    chain_name,
                    n_blocks: NonZero::new(10).unwrap(),
                }
            }
            CliTriggerKind::CosmosBlockInterval => {
                let chain_name = trigger_chain.context("must have trigger chain for contract")?;

                Trigger::BlockInterval {
                    chain_name,
                    n_blocks: NonZero::new(10).unwrap(),
                }
            }
        };

        let submit: Submit = match submit {
            CliSubmitKind::EthServiceHandler => {
                let chain_name = submit_chain.expect("must have submit chain for contract");
                let address = submit_address
                    .context("must have submit address")?
                    .parse()?;

                Submit::EthereumContract {
                    chain_name,
                    address,
                    max_gas: None,
                }
            }

            CliSubmitKind::None => Submit::None,
        };

        let http_client = HttpClient::new(ctx.config.wavs_endpoint.clone());

        let service_config = service_config.unwrap_or_default();

        let service = http_client
            .create_service_simple(
                trigger.clone(),
                submit.clone(),
                component,
                service_config.clone(),
            )
            .await?;

        let _self = Self { args, service };

        _self.update_deployment(&mut ctx.deployment.lock().unwrap());

        Ok(Some(_self))
    }
}
