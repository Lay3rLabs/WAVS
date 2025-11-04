use std::sync::Arc;

use anyhow::{anyhow, Result};
use futures::TryStreamExt;
use wasm_pkg_client::{
    caching::{CachingClient, FileCache},
    Client, Config, Error as WkgError, PackageRef, Release, Version,
};
use wavs_types::{ComponentDigest, Registry};

pub struct WkgClient {
    // due to a bug in the client which can deadlock with the filesystem
    // we want to use a mutex, and hold it across the await point, only releasing when we're done
    // https://github.com/bytecodealliance/wasm-pkg-tools/issues/155
    inner: Arc<tokio::sync::Mutex<InnerWkgClient>>,
}

struct InnerWkgClient {
    client: Option<CachingClient<FileCache>>,
    config: Config,
}

impl WkgClient {
    pub fn new(domain: String) -> Result<Self, WkgError> {
        let config_toml = &format!(
            r#"default_registry = "{domain}"

[registry."wa.dev"]
type = "warg"
[registry."wa.dev".warg]
url = "https://wa.dev"

[registry."localhost:8090"]
type = "warg"
[registry."localhost:8090".warg]
url = "http://localhost:8090"
"#
        );
        let config = Config::from_toml(config_toml)?;
        let inner = Arc::new(tokio::sync::Mutex::new(InnerWkgClient {
            client: None,
            config,
        }));

        Ok(Self { inner })
    }

    /// Helper function to initialize a client with the appropriate domain
    async fn get_client(
        &self,
        domain: Option<&String>,
    ) -> Result<CachingClient<FileCache>, WkgError> {
        let mut inner = self.inner.lock().await;
        let config = &inner.config;

        let client = if domain.is_some() {
            let mut new_config = Config::empty();
            new_config.merge(config.clone());
            // new_config.set_package_registry_override if needed
            let client = Client::new(new_config.clone());
            let cache_path = FileCache::global_cache_path()
                .ok_or_else(|| WkgError::CacheError(anyhow!("couldn't find global cache path")))?;
            let cache = FileCache::new(cache_path)
                .await
                .map_err(WkgError::CacheError)?;
            CachingClient::new(Some(client), cache)
        } else {
            let client = Client::new(config.clone());
            let cache_path = FileCache::global_cache_path()
                .ok_or_else(|| WkgError::CacheError(anyhow!("couldn't find global cache path")))?;
            let cache = FileCache::new(cache_path)
                .await
                .map_err(WkgError::CacheError)?;
            CachingClient::new(Some(client), cache)
        };

        inner.client = Some(client.clone());
        Ok(client)
    }

    /// Helper function to resolve the version to use (provided or latest)
    async fn resolve_version(
        &self,
        client: &CachingClient<FileCache>,
        package: &PackageRef,
        version: Option<&Version>,
    ) -> Result<Version, WkgError> {
        if let Some(v) = version {
            Ok(v.clone())
        } else {
            let mut versions = client.list_all_versions(package).await?;

            versions.sort_by(|a, b| a.version.cmp_precedence(&b.version));
            if versions.is_empty() {
                return Err(WkgError::PackageNotFound);
            }
            Ok(versions[&versions.len() - 1].version.clone())
        }
    }

    /// Helper function to get release information
    async fn get_release(
        &self,
        client: &CachingClient<FileCache>,
        package: &PackageRef,
        version: &Version,
    ) -> Result<Release, WkgError> {
        client.get_release(package, version).await
    }

    /// Helper function to download content and compute digest
    async fn download_and_get_digest(
        &self,
        client: &CachingClient<FileCache>,
        package: &PackageRef,
        release: &Release,
    ) -> Result<(Vec<u8>, ComponentDigest), WkgError> {
        let mut content_stream = client.get_content(package, release).await?;

        let mut content = Vec::new();
        while let Some(chunk) = content_stream.try_next().await? {
            content.append(&mut chunk.to_vec());
        }

        let digest = ComponentDigest::hash(&content);
        Ok((content, digest))
    }

    /// Get the digest for a package from the registry without returning the content
    pub async fn get_digest(
        &self,
        domain: Option<String>,
        package: &PackageRef,
        version: Option<&Version>,
    ) -> Result<(ComponentDigest, Version), WkgError> {
        // Get the client
        let client = self.get_client(domain.as_ref()).await?;

        // Resolve the version
        let resolved_version = self.resolve_version(&client, package, version).await?;

        // Get release info
        let release = self
            .get_release(&client, package, &resolved_version)
            .await?;

        // TODO: If the registry client ever supports retrieving just the digest
        // without downloading content, implement that optimization here

        // Download the content and compute the digest
        let (_, digest) = self
            .download_and_get_digest(&client, package, &release)
            .await?;

        Ok((digest, resolved_version))
    }

    /// First initializes a cache path, needed to instantiate a new client for wkg
    /// (potentially an upstream contribution could alleviate this so a default is used).
    /// Then checks for a user provided version in case they want something other than the default
    /// latest value.
    /// Finally, checks if the user provided an alternative registry other than WAVS default (currently wa.dev),
    /// before fetching the component from the registry.
    pub async fn fetch(&self, registry: &Registry) -> Result<Vec<u8>, WkgError> {
        // Get the client
        let client = self.get_client(registry.domain.as_ref()).await?;

        // Resolve the version
        let resolved_version = self
            .resolve_version(&client, &registry.package, registry.version.as_ref())
            .await?;

        // Get release info
        let release = self
            .get_release(&client, &registry.package, &resolved_version)
            .await?;

        // Download the content and get the digest
        let (content, fetched_digest) = self
            .download_and_get_digest(&client, &registry.package, &release)
            .await?;

        // Verify the digest matches what's expected
        assert_eq!(fetched_digest, registry.digest);

        Ok(content)
    }
}
