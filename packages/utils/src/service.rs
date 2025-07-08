use anyhow::{anyhow, Result};
use cid::Cid;
use reqwest::Client;
use std::str::FromStr;
use url::Url;
use wavs_types::Service;

pub const DEFAULT_IPFS_GATEWAY: &str = "https://ipfs.io/ipfs/";

/// Fetch a Service definition from a URL, handling both HTTP(S) and IPFS URLs
pub async fn fetch_service(url_str: &str, ipfs_gateway: &str) -> Result<Service> {
    // Validate URL first
    let url = Url::parse(url_str)?;

    // Create HTTP client
    let client = Client::new();

    // Determine the actual URL to fetch from
    let fetch_url = match url.scheme() {
        "http" | "https" => url_str.to_string(),
        "ipfs" => ipfs_to_gateway_url(&url, ipfs_gateway)?,
        scheme => {
            return Err(anyhow!(
                "Invalid URL scheme: {}. Only http, https, and ipfs schemes are allowed",
                scheme
            ))
        }
    };

    // Fetch the service definition
    let response = client
        .get(&fetch_url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch service definition: {}", e))?;

    // Check if the request was successful
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch service definition, status code: {}",
            response.status()
        ));
    }

    // Parse the JSON response into a Service
    let service: Service = response
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse service definition as JSON: {}", e))?;

    Ok(service)
}

/// Convert an IPFS URL to an HTTP URL using the specified gateway
pub fn ipfs_to_gateway_url(ipfs_url: &Url, ipfs_gateway: &str) -> Result<String> {
    // Verify the URL uses the IPFS scheme
    if ipfs_url.scheme() != "ipfs" {
        return Err(anyhow!("URL is not an IPFS URL"));
    }

    // Extract the CID from the host part
    let host = ipfs_url
        .host_str()
        .ok_or_else(|| anyhow!("IPFS URL must have a host"))?;

    // Validate the CID
    let cid = Cid::from_str(host).map_err(|_| anyhow!("Invalid IPFS CID in host: {}", host))?;

    // Build the gateway URL: gateway + CID + path
    let path = ipfs_url.path().trim_start_matches('/');
    if path.is_empty() {
        Ok(format!("{ipfs_gateway}{cid}"))
    } else {
        Ok(format!("{ipfs_gateway}{cid}/{path}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipfs_to_gateway_url_valid_cid_only() {
        let url = Url::parse("ipfs://bafybeigdyrzt6f5s7z5qvu2xrbqopxlh2psrbgn3cz3he4eug3pynkq2uq")
            .unwrap();
        let result = ipfs_to_gateway_url(&url, DEFAULT_IPFS_GATEWAY).unwrap();

        assert_eq!(
            result,
            format!(
                "{}{}",
                DEFAULT_IPFS_GATEWAY, "bafybeigdyrzt6f5s7z5qvu2xrbqopxlh2psrbgn3cz3he4eug3pynkq2uq"
            )
        );
    }

    #[test]
    fn test_ipfs_to_gateway_url_valid_with_path() {
        let url = Url::parse(
            "ipfs://bafybeigdyrzt6f5s7z5qvu2xrbqopxlh2psrbgn3cz3he4eug3pynkq2uq/assets/logo.png",
        )
        .unwrap();
        let result = ipfs_to_gateway_url(&url, DEFAULT_IPFS_GATEWAY).unwrap();

        assert_eq!(
            result,
            format!(
                "{}{}/{}",
                DEFAULT_IPFS_GATEWAY,
                "bafybeigdyrzt6f5s7z5qvu2xrbqopxlh2psrbgn3cz3he4eug3pynkq2uq",
                "assets/logo.png"
            )
        );
    }

    #[test]
    fn test_ipfs_to_gateway_url_invalid_scheme() {
        let url = Url::parse("https://bafybeigdyrzt6f5s7z5qvu2xrbqopxlh2psrbgn3cz3he4eug3pynkq2uq")
            .unwrap();
        let result = ipfs_to_gateway_url(&url, DEFAULT_IPFS_GATEWAY);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "URL is not an IPFS URL");
    }

    #[test]
    fn test_ipfs_to_gateway_url_missing_host() {
        let url = Url::parse("ipfs:///some/path").unwrap();
        let result = ipfs_to_gateway_url(&url, DEFAULT_IPFS_GATEWAY);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "IPFS URL must have a host");
    }

    #[test]
    fn test_ipfs_to_gateway_url_invalid_cid() {
        let url = Url::parse("ipfs://not-a-valid-cid/path/to/file").unwrap();
        let result = ipfs_to_gateway_url(&url, DEFAULT_IPFS_GATEWAY);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid IPFS CID"));
    }
}
