use std::ops::Bound;

use crate::AppContext;
use wavs_types::{ComponentSource, Digest, Service, ServiceID};

/// This is the highest-level container for the system.
/// The http server can hold this in state and interact with the "management interface".
/// The other components route to each other via this one.
///
/// It uses internal mutability pattern, so we can have multiple references to it.
/// It should implement Send and Sync so it can be used in async code.
///
/// These types should not be raw from the user, but parsed from the JSON structs, validated,
/// and converted into our internal structs
pub trait DispatchManager: Send + Sync {
    type Error;

    fn start(&self, ctx: AppContext) -> Result<(), Self::Error>;

    /// Used to install new wasm bytecode into the system.
    /// Either the bytecode is provided directly, or it is downloaded from a URL.
    fn store_component(&self, source: ComponentSource) -> Result<Digest, Self::Error>;

    fn add_service(&self, service: Service) -> Result<(), Self::Error>;

    fn remove_service(&self, id: ServiceID) -> Result<(), Self::Error>;

    fn list_services(
        &self,
        bounds_start: Bound<&str>,
        bounds_end: Bound<&str>,
    ) -> Result<Vec<Service>, Self::Error>;

    /// TODO: pagination
    fn list_component_digests(&self) -> Result<Vec<Digest>, Self::Error>;

    // TODO: this would be nicer so we can just pass in a range
    // but then we run into problems with storing DispatchManager as a trait object
    // fn list_services<'a>(&self, bounds: impl RangeBounds<&'a str>) -> Result<Vec<Service>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use wavs_types::{AllowedHostPermission, Permissions};

    #[test]
    fn permission_defaults() {
        let permissions_json: Permissions = serde_json::from_str("{}").unwrap();
        let permissions_default: Permissions = Permissions::default();

        assert_eq!(permissions_json, permissions_default);
        assert_eq!(
            permissions_default.allowed_http_hosts,
            AllowedHostPermission::None
        );
        assert!(!permissions_default.file_system);
    }
}
