use crate::{clients::HttpClient, context::CliContext, deploy::CommandDeployResult};
use anyhow::Result;
use wavs_types::Service;

pub struct DeployServiceRaw {
    pub args: DeployServiceRawArgs,
}

impl std::fmt::Display for DeployServiceRaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "New Service deployed to wavs")?;
        write!(f, "\n\n{:#?}", self.args.service)
    }
}

impl CommandDeployResult for DeployServiceRaw {
    fn update_deployment(&self, deployment: &mut crate::deploy::Deployment) {
        deployment
            .services
            .insert(self.args.service.id.clone(), self.args.service.clone());
    }
}

#[derive(Clone)]
pub struct DeployServiceRawArgs {
    pub service: Service,
}

impl DeployServiceRaw {
    pub async fn run(ctx: &CliContext, args: DeployServiceRawArgs) -> Result<Self> {
        let service = args.service.clone();

        let http_client = HttpClient::new(ctx.config.wavs_endpoint.clone());

        http_client.create_service_raw(&ctx.config, service).await?;

        let _self = Self { args };

        _self.update_deployment(&mut ctx.deployment.lock().unwrap());

        Ok(_self)
    }
}
