use anyhow::{Context, Result};
use reqwest::{Client, Method};
use serde_json::Value;
use wavs_types::{AddServiceRequest, PauseServiceRequest, ServiceManager, SimulatedTriggerRequest, UploadComponentResponse};

#[derive(Clone)]
pub struct WavsClient {
    inner: Client,
    endpoint: String,
    token: Option<String>,
}

impl WavsClient {
    pub fn new(endpoint: String, token: Option<String>) -> Self {
        Self {
            inner: Client::new(),
            endpoint: endpoint.trim_end_matches('/').to_string(),
            token,
        }
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.endpoint, path);
        let mut req = self.inner.request(method, url);
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }
        req
    }

    pub async fn get_info(&self) -> Result<Value> {
        let resp = self.request(Method::GET, "/info")
            .send()
            .await
            .context("GET /info")?;
        parse_json_response(resp).await
    }

    pub async fn get_health(&self) -> Result<Value> {
        let resp = self.request(Method::GET, "/health")
            .send()
            .await
            .context("GET /health")?;
        parse_json_response(resp).await
    }

    pub async fn list_services(&self) -> Result<Value> {
        let resp = self.request(Method::GET, "/services")
            .send()
            .await
            .context("GET /services")?;
        parse_json_response(resp).await
    }

    pub async fn get_service(&self, chain: &str, address: &str) -> Result<Value> {
        let path = format!("/services/{}/{}", chain, address);
        let resp = self.request(Method::GET, &path)
            .send()
            .await
            .context("GET /services/{chain}/{address}")?;
        parse_json_response(resp).await
    }

    pub async fn deploy_service(&self, service_manager: ServiceManager) -> Result<Value> {
        let body = serde_json::to_string(&AddServiceRequest { service_manager })?;
        let resp = self.request(Method::POST, "/services")
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .context("POST /services")?;
        parse_json_response(resp).await
    }

    pub async fn pause_service(&self, service_manager: ServiceManager) -> Result<()> {
        let body = serde_json::to_string(&PauseServiceRequest { service_manager })?;
        let resp = self.request(Method::POST, "/services/pause")
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .context("POST /services/pause")?;
        check_response(resp).await
    }

    pub async fn resume_service(&self, service_manager: ServiceManager) -> Result<()> {
        let body = serde_json::to_string(&PauseServiceRequest { service_manager })?;
        let resp = self.request(Method::POST, "/services/resume")
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .context("POST /services/resume")?;
        check_response(resp).await
    }

    pub async fn upload_component(&self, bytes: Vec<u8>) -> Result<String> {
        let resp = self.request(Method::POST, "/dev/components")
            .body(bytes)
            .send()
            .await
            .context("POST /dev/components")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("HTTP {}: {}", status, body);
        }

        let response: UploadComponentResponse = resp.json().await?;
        Ok(response.digest.to_string())
    }

    pub async fn simulate_trigger(&self, req: SimulatedTriggerRequest) -> Result<()> {
        let resp = self.request(Method::POST, "/dev/triggers")
            .json(&req)
            .send()
            .await
            .context("POST /dev/triggers")?;
        check_response(resp).await
    }
}

async fn parse_json_response(resp: reqwest::Response) -> Result<Value> {
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("HTTP {}: {}", status, body);
    }
    resp.json().await.map_err(Into::into)
}

async fn check_response(resp: reqwest::Response) -> Result<()> {
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("HTTP {}: {}", status, body);
    }
    Ok(())
}
