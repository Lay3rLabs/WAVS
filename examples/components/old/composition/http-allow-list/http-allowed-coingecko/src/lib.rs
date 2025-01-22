#[allow(warnings)]
mod bindings;

use bindings::exports::wasi::http::outgoing_handler::{
    ErrorCode, FutureIncomingResponse, Guest, OutgoingRequest, RequestOptions,
};

/// Allowed authorities for outgoing HTTP requests.
const ALLOWED: [&str; 1] = ["api.coingecko.com"];

struct Component;

impl Guest for Component {
    fn handle(
        request: OutgoingRequest,
        options: Option<RequestOptions>,
    ) -> Result<FutureIncomingResponse, ErrorCode> {
        // if is in allowed list then make the request, otherwise refuse connection
        match request.authority() {
            Some(authority) if ALLOWED.contains(&authority.to_lowercase().as_str()) => {
                bindings::wasi::http::outgoing_handler::handle(request, options)
            }
            _ => Err(ErrorCode::ConnectionRefused),
        }
    }
}

bindings::export!(Component with_types_in bindings);
