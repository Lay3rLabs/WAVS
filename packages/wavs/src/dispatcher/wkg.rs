use std::sync::Arc;

use anyhow::{Context, Result};
use futures::TryStreamExt;
use wasm_pkg_client::{
    caching::{CachingClient, FileCache},
    Client, Config,
};
use wavs_types::Registry;

pub struct WkgClient {
    // due to a bug in the client which can deadlock with the filesystem
    // we want to use a mutex, and hold it across the await point, only releasing when we're done
    inner: Arc<tokio::sync::Mutex<InnerWkgClient>>,
}

struct InnerWkgClient {
    client: Option<CachingClient<FileCache>>,
    config: Config,
}

impl WkgClient {
    pub fn new(domain: String) -> Result<Self> {
        let config_toml = &format!("default_registry = \"{domain}\"");
        let config = Config::from_toml(config_toml)?;
        let inner = Arc::new(tokio::sync::Mutex::new(InnerWkgClient {
            client: None,
            config,
        }));

        Ok(Self { inner })
    }

    /// First initializes a cache path, needed to instantiate a new client for wkg
    /// (potentially an upstream contribution could alleviate this so a default is used).
    /// Then checks for a user provided version in case they want something other than the default
    /// latest value.
    /// Finally, checks if the user provided an alternative registry other than WAVS default (currently wa.dev),
    /// before fetching the component from the registry.
    pub async fn fetch(&self, registry: Registry) -> Result<Vec<u8>> {
        let mut inner = self.inner.lock().await;
        let config = &inner.config;
        let client = if let Some(domain) = registry.domain {
            let mut new_config = Config::empty();
            new_config.merge(config.clone());
            new_config.set_package_registry_override(registry.package.clone(), domain.clone());
            let client = Client::new(new_config.clone());
            let cache_path =
                FileCache::global_cache_path().context("couldn't find global cache path")?;
            let cache = FileCache::new(cache_path).await?;
            let client = CachingClient::new(Some(client), cache);
            inner.client = Some(client.clone());
            client
        } else {
            let client = Client::new(config.clone());
            let cache_path =
                FileCache::global_cache_path().context("couldn't find global cache path")?;
            let cache = FileCache::new(cache_path).await?;
            let client = CachingClient::new(Some(client), cache);
            inner.client = Some(client.clone());
            client
        };
        let version = if let Some(v) = &registry.version {
            v.clone()
        } else {
            let mut versions = client.list_all_versions(&registry.package).await?;
            versions.sort_by(|a, b| a.version.cmp_precedence(&b.version));
            versions[&versions.len() - 1].version.clone()
        };

        let release = client.get_release(&registry.package, &version).await?;
        let mut content_stream = client.get_content(&registry.package, &release).await?;
        let mut content = Vec::new();
        while let Some(chunk) = content_stream.try_next().await? {
            content.append(&mut chunk.to_vec());
        }
        Ok(content)
    }
}
