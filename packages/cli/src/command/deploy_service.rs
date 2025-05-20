use crate::{clients::HttpClient, context::CliContext, deploy::CommandDeployResult};
use alloy_provider::DynProvider;
use anyhow::Result;
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
            .insert(self.args.service.id.clone(), self.args.service.clone());
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
        let service_id = service.id.clone();

        let http_client = HttpClient::new(ctx.config.wavs_endpoint.clone());

        match http_client
            .create_service(service, args.set_service_url_args.clone())
            .await
        {
            Ok(_) => {}
            Err(err) => {
                // Extract the underlying error message for better context
                let error_context = format!(
                    "Failed to deploy service with ID '{}' to endpoint '{}'\nReason: {}",
                    service_id, ctx.config.wavs_endpoint, err
                );
                return Err(anyhow::anyhow!(error_context));
            }
        };

        let _self = Self { args };

        _self.update_deployment(&mut ctx.deployment.lock().unwrap());

        Ok(_self)
    }

    pub async fn save_service(ctx: &CliContext, service: &Service) -> Result<String> {
        let http_client = HttpClient::new(ctx.config.wavs_endpoint.clone());

        http_client.save_service(service).await
    }
}
