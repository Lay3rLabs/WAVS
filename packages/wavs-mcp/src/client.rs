use anyhow::{Context, Result};
use reqwest::{Client, Method};
use serde_json::Value;
use wavs_types::{
    AddServiceRequest, DeleteServicesRequest, GetSignerRequest, SaveServiceResponse,
    ServiceManager, SignerResponse, SimulatedTriggerRequest, UploadComponentResponse,
};

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
        let resp = self
            .request(Method::GET, "/info")
            .send()
            .await
            .context("GET /info")?;
        parse_json_response(resp).await
    }

    pub async fn get_chains(&self) -> Result<Value> {
        let resp = self
            .request(Method::GET, "/chains")
            .send()
            .await
            .context("GET /chains")?;
        parse_json_response(resp).await
    }

    pub async fn get_health(&self) -> Result<Value> {
        let resp = self
            .request(Method::GET, "/health")
            .send()
            .await
            .context("GET /health")?;
        parse_json_response(resp).await
    }

    pub async fn list_services(&self) -> Result<Value> {
        let resp = self
            .request(Method::GET, "/services")
            .send()
            .await
            .context("GET /services")?;
        parse_json_response(resp).await
    }

    pub async fn get_service(&self, chain: &str, address: &str) -> Result<Value> {
        let path = format!("/services/{}/{}", chain, address);
        let resp = self
            .request(Method::GET, &path)
            .send()
            .await
            .context("GET /services/{chain}/{address}")?;
        parse_json_response(resp).await
    }

    pub async fn deploy_service(&self, service_manager: ServiceManager) -> Result<Value> {
        let body = serde_json::to_string(&AddServiceRequest { service_manager })?;
        let resp = self
            .request(Method::POST, "/services")
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .context("POST /services")?;
        parse_json_response(resp).await
    }

    pub async fn delete_service(&self, service_manager: ServiceManager) -> Result<()> {
        let body = serde_json::to_string(&DeleteServicesRequest {
            service_managers: vec![service_manager],
        })?;
        let resp = self
            .request(Method::DELETE, "/services")
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .context("DELETE /services")?;
        check_response(resp).await
    }

    pub async fn upload_component(&self, bytes: Vec<u8>) -> Result<String> {
        let resp = self
            .request(Method::POST, "/dev/components")
            .body(bytes)
            .send()
            .await
            .context("POST /dev/components")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(dev_err(status, &body));
        }

        let response: UploadComponentResponse = resp.json().await?;
        Ok(response.digest.to_string())
    }

    pub async fn simulate_trigger(&self, req: SimulatedTriggerRequest) -> Result<()> {
        let resp = self
            .request(Method::POST, "/dev/triggers")
            .json(&req)
            .send()
            .await
            .context("POST /dev/triggers")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(dev_err(status, &body));
        }
        Ok(())
    }

    /// Save a service definition to the node's local store without registering it.
    /// Returns the URI at which the service JSON can be fetched.
    pub async fn save_service(&self, service_json: &str) -> Result<String> {
        let resp = self
            .request(Method::POST, "/dev/services")
            .header("Content-Type", "application/json")
            .body(service_json.to_string())
            .send()
            .await
            .context("POST /dev/services")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(dev_err(status, &body));
        }

        let save_resp: SaveServiceResponse = resp.json().await?;
        let hash = save_resp.hash.to_string();
        Ok(format!("{}/dev/services/{}", self.endpoint, hash))
    }

    /// Two-step dev registration: POST /dev/services → save, POST /dev/services/{hash} → register.
    /// Returns the service hash on success.
    pub async fn deploy_dev_service(&self, service_json: &str) -> Result<String> {
        let resp = self
            .request(Method::POST, "/dev/services")
            .header("Content-Type", "application/json")
            .body(service_json.to_string())
            .send()
            .await
            .context("POST /dev/services")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(dev_err(status, &body));
        }

        let save_resp: SaveServiceResponse = resp.json().await?;
        let hash = save_resp.hash.to_string();

        let path = format!("/dev/services/{}", hash);
        let resp = self
            .request(Method::POST, &path)
            .send()
            .await
            .context("POST /dev/services/{hash}")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(dev_err(status, &body));
        }

        Ok(hash)
    }

    /// POST /services/signer — returns the signing key info for a service.
    pub async fn get_service_signer(
        &self,
        service_manager: ServiceManager,
    ) -> Result<SignerResponse> {
        let body = serde_json::to_string(&GetSignerRequest { service_manager })?;
        let resp = self
            .request(Method::POST, "/services/signer")
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .context("POST /services/signer")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("HTTP {}: {}", status, body);
        }
        resp.json().await.context("parse SignerResponse")
    }

    /// GET /dev/logs — poll structured log entries from the in-memory ring buffer.
    pub async fn query_logs(
        &self,
        since_id: u64,
        limit: Option<usize>,
        level: Option<&str>,
        target: Option<&str>,
    ) -> Result<Value> {
        let mut params: Vec<(&str, String)> = vec![("since_id", since_id.to_string())];
        if let Some(l) = limit {
            params.push(("limit", l.to_string()));
        }
        if let Some(lv) = level {
            params.push(("level", lv.to_string()));
        }
        if let Some(t) = target {
            params.push(("target", t.to_string()));
        }
        let resp = self
            .request(Method::GET, "/dev/logs")
            .query(&params)
            .send()
            .await
            .context("GET /dev/logs")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(dev_err(status, &body));
        }
        resp.json().await.context("parse logs response")
    }

    /// GET /dev/kv/{service_id}/{bucket}/{key} — returns value as UTF-8 or hex.
    pub async fn query_kv(&self, service_id: &str, bucket: &str, key: &str) -> Result<String> {
        let path = format!("/dev/kv/{}/{}/{}", service_id, bucket, key);
        let resp = self
            .request(Method::GET, &path)
            .send()
            .await
            .context("GET /dev/kv")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(dev_err(status, &body));
        }

        let bytes = resp.bytes().await?;
        Ok(match std::str::from_utf8(&bytes) {
            Ok(s) => s.to_string(),
            Err(_) => const_hex::encode(&bytes),
        })
    }

    /// POST /dev/execute -- synchronously execute a component and return results.
    ///
    /// Calls the WAVS node's `/dev/execute` endpoint which bypasses the full
    /// trigger/aggregator/submission pipeline and returns the raw component output.
    pub async fn execute_component(
        &self,
        service_id: &str,
        workflow_id: &str,
        trigger_json: &serde_json::Value,
        data_json: &serde_json::Value,
    ) -> Result<Vec<serde_json::Value>> {
        let body = serde_json::json!({
            "service_id": service_id,
            "workflow_id": workflow_id,
            "trigger": trigger_json,
            "data": data_json,
        });
        let resp = self
            .request(Method::POST, "/dev/execute")
            .json(&body)
            .send()
            .await
            .context("POST /dev/execute")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(dev_err(status, &body));
        }
        resp.json().await.context("parse execute response")
    }
}

fn dev_err(status: reqwest::StatusCode, body: &str) -> anyhow::Error {
    if status.as_u16() == 404 {
        anyhow::anyhow!(
            "HTTP 404: {}. Dev endpoints may be disabled — set dev_endpoints_enabled = true \
             in wavs.toml [wavs] section.",
            body
        )
    } else {
        anyhow::anyhow!("HTTP {}: {}", status, body)
    }
}

async fn parse_json_response(resp: reqwest::Response) -> Result<Value> {
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("HTTP {}: {}", status, body);
    }
    let bytes = resp.bytes().await?;
    if bytes.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&bytes).map_err(Into::into)
}

async fn check_response(resp: reqwest::Response) -> Result<()> {
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("HTTP {}: {}", status, body);
    }
    Ok(())
}
