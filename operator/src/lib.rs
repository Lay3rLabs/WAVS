use std::io::{BufWriter, Write};

use serde::{Deserialize, Serialize};
#[allow(warnings)]
mod bindings;

use bindings::exports::wasi::http::incoming_handler::{Guest, IncomingRequest, ResponseOutparam};
use bindings::wasi::http::types::{Fields, OutgoingBody, OutgoingResponse};

struct Component;

impl Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let method = request.method();
        match method {
            bindings::wasi::http::types::Method::Post => {
                let path = request
                    .path_with_query()
                    .expect("Failed to read request path");
                let mut path_parts = path.split("?");
                let path = path_parts.next().unwrap();

                if path == "/register" {
                    let query = path_parts.next().unwrap();
                    let val = query.split("=").last().unwrap();
                    let req_body = request.consume().unwrap();
                    let stream = req_body.stream().unwrap();
                    let file = std::fs::File::create(format!("./registered/{val}.wasm")).unwrap();

                    let mut more_bytes = true;
                    let mut write_stream = BufWriter::new(file);
                    while more_bytes {
                        let bytes = stream.blocking_read(10000000000000000000);
                        if let Ok(b) = bytes {
                            write_stream.write(&b).unwrap();
                        } else {
                            more_bytes = false;
                        }
                    }
                }
                dbg!("WROTE TO FILE");
            }
            _ => {}
        }
        let hdrs = Fields::new();
        let resp = OutgoingResponse::new(hdrs);
        let body = resp.body().expect("outgoing response");
        let out = body.write().expect("outgoing stream");
        out.blocking_write_and_flush("hello world".as_bytes())
            .unwrap();
        ResponseOutparam::set(response_out, Ok(resp));

        drop(out);
        OutgoingBody::finish(body, None).unwrap();
    }
}

bindings::export!(Component with_types_in bindings);
