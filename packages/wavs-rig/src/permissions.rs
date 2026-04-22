//! Startup validation for WAVS agent HTTP permissions.
//!
//! Agents require HTTP access to reach LLM providers. This module provides
//! a check that returns a clear error instead of a silent WASI trap.

/// Permission level for outbound HTTP from a WAVS component.
///
/// This mirrors `AllowedHostPermission` from packages/types/src/service.rs.
/// The component passes the permission from `host::get_service()` since
/// wavs-rig is an rlib and cannot call WIT host functions directly.
#[derive(Debug, Clone)]
pub enum HttpPermission {
    /// All outbound HTTP allowed
    All,
    /// No outbound HTTP allowed
    None,
    /// Only specific hosts allowed
    Only(Vec<String>),
}

/// Check that the component has HTTP access for LLM API calls.
///
/// Call this at agent startup before attempting any LLM requests.
/// Pass the permission extracted from `host::get_service().service.permissions.allowed_http_hosts`.
///
/// Returns Ok(()) if HTTP is available, or Err with a human-readable message.
pub fn check_http_permission(permission: &HttpPermission) -> Result<(), String> {
    match permission {
        HttpPermission::All | HttpPermission::Only(_) => Ok(()),
        HttpPermission::None => Err(
            "WAVS agent requires HTTP access \
             — set AllowedHostPermission to All or Only"
                .to_string(),
        ),
    }
}
