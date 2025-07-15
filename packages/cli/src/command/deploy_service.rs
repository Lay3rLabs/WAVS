use crate::{clients::HttpClient, context::CliContext, deploy::CommandDeployResult};
use alloy_provider::DynProvider;
use anyhow::{Context, Result};
use wavs_types::Service;

pub struct DeployService {
    pub args: DeployServiceArgs,
}

impl std::fmt::Display for DeployService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "New Service deployed to wavs")?;
        if let Some(save_service_args) = &self.args.set_service_url_args {
            write!(f, "\n\n{:#?}", save_service_args.service_url)?;
        }
        write!(f, "\n\n{:#?}", self.args.service)
    }
}

impl CommandDeployResult for DeployService {
    fn update_deployment(&self, deployment: &mut crate::deploy::Deployment) {
        deployment
            .services
            .insert(self.args.service.id(), self.args.service.clone());
    }
}

#[derive(Clone)]
pub struct DeployServiceArgs {
    pub service: Service,
    pub set_service_url_args: Option<SetServiceUrlArgs>,
}

#[derive(Clone)]
pub struct SetServiceUrlArgs {
    pub provider: DynProvider,
    pub service_url: String,
}

impl DeployService {
    pub async fn run(ctx: &CliContext, args: DeployServiceArgs) -> Result<Self> {
        let service = args.service.clone();
        let service_id = service.id();

        let http_client = HttpClient::new(ctx.config.wavs_endpoint.clone());

        http_client
            .create_service(service, args.set_service_url_args.clone())
            .await
            .context(format!("Failed to deploy service with ID '{}'", service_id))?;

        let _self = Self { args };

        _self.update_deployment(&mut ctx.deployment.lock().unwrap());

        Ok(_self)
    }

    pub async fn save_service(ctx: &CliContext, service: &Service) -> Result<String> {
        let http_client = HttpClient::new(ctx.config.wavs_endpoint.clone());

        http_client.save_service(service).await
    }
}
