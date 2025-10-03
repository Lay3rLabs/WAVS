use anyhow::{anyhow, Result};
use cid::Cid;
use iri_string::types::UriStr;
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::str::FromStr;
use wavs_types::Service;

pub const DEFAULT_IPFS_GATEWAY: &str = "https://ipfs.io/ipfs/";

/// Fetch a Service definition from a URI, handling both HTTP(S) and IPFS URIs
pub async fn fetch_service(uri: &UriStr, ipfs_gateway: &str) -> Result<Service> {
    fetch_json::<Service>(uri, ipfs_gateway).await
}

/// Internal helper to fetch data from a URI, handling both HTTP(S) and IPFS URIs
async fn fetch_response(uri: &UriStr, ipfs_gateway: &str) -> Result<reqwest::Response> {
    // Create HTTP client
    let client = Client::new();

    // Determine the actual URL to fetch from
    let fetch_url = match uri.scheme_str() {
        "http" | "https" => uri.to_string(),
        "ipfs" => ipfs_to_gateway_url(uri, ipfs_gateway)?,
        scheme => {
            return Err(anyhow!(
            "Unsupported URI scheme: {}. Currently supported schemes are: http, https, and ipfs",
            scheme
        ))
        }
    };

    // Fetch the data
    let response = client
        .get(&fetch_url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch data: {}", e))?;

    // Check if the request was successful
    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch data, status code: {}",
            response.status()
        ));
    }

    Ok(response)
}

/// Fetch a JSON deserializable type from a URI, handling both HTTP(S) and IPFS URIs
pub async fn fetch_json<T: DeserializeOwned>(uri: &UriStr, ipfs_gateway: &str) -> Result<T> {
    let response = fetch_response(uri, ipfs_gateway).await?;

    // Parse the JSON response into the target type
    let data: T = response
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse response as JSON: {}", e))?;

    Ok(data)
}

/// Fetch raw bytes from a URI, handling both HTTP(S) and IPFS URIs
pub async fn fetch_bytes(uri: &UriStr, ipfs_gateway: &str) -> Result<Vec<u8>> {
    let response = fetch_response(uri, ipfs_gateway).await?;

    // Get the raw bytes
    let bytes = response
        .bytes()
        .await
        .map_err(|e| anyhow!("Failed to read response bytes: {}", e))?;

    Ok(bytes.to_vec())
}

/// Convert an IPFS URI to an HTTP URL using the specified gateway
pub fn ipfs_to_gateway_url(ipfs_uri: &UriStr, ipfs_gateway: &str) -> Result<String> {
    // Verify the URI uses the IPFS scheme
    if ipfs_uri.scheme_str() != "ipfs" {
        return Err(anyhow!("URI is not an IPFS URI"));
    }

    // Extract the CID from the authority part
    let authority = ipfs_uri
        .authority_str()
        .ok_or_else(|| anyhow!("IPFS URI must have an authority"))?;

    // Validate the CID
    let cid = Cid::from_str(authority)
        .map_err(|_| anyhow!("Invalid IPFS CID in authority: {}", authority))?;

    // Build the gateway URL: gateway + CID + path
    let path = ipfs_uri.path_str().trim_start_matches('/');
    if path.is_empty() {
        Ok(format!("{ipfs_gateway}{cid}"))
    } else {
        Ok(format!("{ipfs_gateway}{cid}/{path}"))
    }
}

#[cfg(test)]
mod tests {
    use iri_string::types::UriString;

    use super::*;

    #[test]
    fn test_ipfs_to_gateway_url_valid_cid_only() {
        let uri = UriString::try_from(
            "ipfs://bafybeigdyrzt6f5s7z5qvu2xrbqopxlh2psrbgn3cz3he4eug3pynkq2uq",
        )
        .unwrap();
        let result = ipfs_to_gateway_url(&uri, DEFAULT_IPFS_GATEWAY).unwrap();

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
        let uri = UriString::try_from(
            "ipfs://bafybeigdyrzt6f5s7z5qvu2xrbqopxlh2psrbgn3cz3he4eug3pynkq2uq/assets/logo.png",
        )
        .unwrap();
        let result = ipfs_to_gateway_url(&uri, DEFAULT_IPFS_GATEWAY).unwrap();

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
        let uri = UriString::try_from(
            "https://bafybeigdyrzt6f5s7z5qvu2xrbqopxlh2psrbgn3cz3he4eug3pynkq2uq",
        )
        .unwrap();
        let result = ipfs_to_gateway_url(&uri, DEFAULT_IPFS_GATEWAY);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "URI is not an IPFS URI");
    }

    #[test]
    fn test_ipfs_to_gateway_url_missing_authority() {
        let uri = UriString::try_from("ipfs:///some/path").unwrap();
        let result = ipfs_to_gateway_url(&uri, DEFAULT_IPFS_GATEWAY);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid IPFS CID in authority"));
    }

    #[test]
    fn test_ipfs_to_gateway_url_invalid_cid() {
        let uri = UriString::try_from("ipfs://not-a-valid-cid/path/to/file").unwrap();
        let result = ipfs_to_gateway_url(&uri, DEFAULT_IPFS_GATEWAY);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid IPFS CID"));
    }
}
