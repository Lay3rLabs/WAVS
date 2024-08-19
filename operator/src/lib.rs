use serde::{Deserialize, Serialize};
#[allow(warnings)]
mod bindings;

use bindings::exports::wasi::http::incoming_handler::{Guest, IncomingRequest, ResponseOutparam};
use bindings::wasi::http::types::{Fields, OutgoingBody, OutgoingResponse};

#[derive(Debug, Serialize, Deserialize)]
struct Body {
    function: String,
    key: Option<String>,
}
struct Component;

impl Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let hdrs = Fields::new();
        let resp = OutgoingResponse::new(hdrs);
        let body = resp.body().expect("outgoing response");
        let out = body.write().expect("outgoing stream");
        // out.blocking_write_and_flush(&res.as_bytes())
        //     .expect("writing response");
        out.blocking_write_and_flush("hello world".as_bytes())
            .unwrap();
        ResponseOutparam::set(response_out, Ok(resp));

        drop(out);
        OutgoingBody::finish(body, None).unwrap();
    }
}

bindings::export!(Component with_types_in bindings);
