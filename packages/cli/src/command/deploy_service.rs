use crate::{clients::HttpClient, context::CliContext, deploy::CommandDeployResult};
use alloy_provider::Provider;
use anyhow::Result;
use wavs_types::Service;

pub struct DeployService {
    pub args: DeployServiceArgs,
}

impl std::fmt::Display for DeployService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "New Service deployed to wavs")?;
        write!(f, "\n\n{:#?}", self.args.service_url)?;
        write!(f, "\n\n{:#?}", self.args.service)
    }
}

impl CommandDeployResult for DeployService {
    fn update_deployment(&self, deployment: &mut crate::deploy::Deployment) {
        deployment
            .services
            .insert(self.args.service.id.clone(), self.args.service.clone());
    }
}

#[derive(Clone)]
pub struct DeployServiceArgs {
    pub service: Service,
    pub service_url: Option<String>,
}

impl DeployService {
    pub async fn run<T: Provider>(
        ctx: &CliContext,
        provider: T,
        args: DeployServiceArgs,
    ) -> Result<Self> {
        let service = args.service.clone();

        let http_client = HttpClient::new(ctx.config.wavs_endpoint.clone());

        http_client
            .create_service(provider, service, args.service_url.clone())
            .await?;

        let _self = Self { args };

        _self.update_deployment(&mut ctx.deployment.lock().unwrap());

        Ok(_self)
    }
}
