use crate::{clients::HttpClient, context::CliContext, deploy::CommandDeployResult};
use alloy_provider::DynProvider;
use anyhow::{Context, Result};
use iri_string::types::UriString;
use layer_climb::signing::SigningClient;
use wavs_types::{Service, ServiceManager};

pub struct DeployService {
    pub args: DeployServiceArgs,
    pub service: Service,
}

impl std::fmt::Display for DeployService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "New Service deployed to wavs")?;
        if let Some(save_service_args) = &self.args.set_service_url_args {
            write!(f, "\n\n{:#?}", save_service_args.service_uri())?;
        }
        write!(f, "\n\n{:#?}", self.args.service_manager)
    }
}

impl CommandDeployResult for DeployService {
    fn update_deployment(&self, deployment: &mut crate::deploy::Deployment) {
        deployment
            .services
            .insert(self.service.id(), self.service.clone());
    }
}

#[derive(Clone)]
pub struct DeployServiceArgs {
    pub service_manager: ServiceManager,
    pub set_service_url_args: Option<SetServiceUriArgs>,
}

#[derive(Clone)]
pub enum SetServiceUriArgs {
    Evm {
        provider: DynProvider,
        service_uri: UriString,
    },
    Cosmos {
        client: SigningClient,
        service_uri: UriString,
    },
}

impl SetServiceUriArgs {
    pub fn new_evm(provider: DynProvider, service_uri: UriString) -> Self {
        Self::Evm {
            provider,
            service_uri,
        }
    }

    pub fn new_cosmos(client: SigningClient, service_uri: UriString) -> Self {
        Self::Cosmos {
            client,
            service_uri,
        }
    }

    pub fn service_uri(&self) -> &UriString {
        match self {
            SetServiceUriArgs::Evm { service_uri, .. } => service_uri,
            SetServiceUriArgs::Cosmos { service_uri, .. } => service_uri,
        }
    }
}

impl DeployService {
    pub async fn run(ctx: &CliContext, args: DeployServiceArgs) -> Result<Self> {
        let service_manager = args.service_manager.clone();

        let http_client = HttpClient::new(ctx.config.wavs_endpoint.clone());

        let service = http_client
            .create_service(service_manager.clone(), args.set_service_url_args.clone())
            .await
            .context(format!(
                "Failed to deploy service with '{:?}'",
                service_manager
            ))?;

        let _self = Self { args, service };

        _self.update_deployment(&mut ctx.deployment.lock().unwrap());

        Ok(_self)
    }

    pub async fn save_service(ctx: &CliContext, service: &Service) -> Result<String> {
        let http_client = HttpClient::new(ctx.config.wavs_endpoint.clone());

        http_client.save_service(service).await
    }
}
