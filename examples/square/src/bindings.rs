pub type TaskQueueInput = lay3r::avs::types::TaskQueueInput;
pub type Output = lay3r::avs::types::Output;
#[doc(hidden)]
#[allow(non_snake_case)]
pub unsafe fn _export_run_task_cabi<T: Guest>(arg0: i64, arg1: *mut u8, arg2: usize) -> *mut u8 {
    #[cfg(target_arch = "wasm32")]
    _rt::run_ctors_once();
    let len0 = arg2;
    let result1 = T::run_task(lay3r::avs::types::TaskQueueInput {
        timestamp: arg0 as u64,
        request: _rt::Vec::from_raw_parts(arg1.cast(), len0, len0),
    });
    let ptr2 = _RET_AREA.0.as_mut_ptr().cast::<u8>();
    match result1 {
        Ok(e) => {
            *ptr2.add(0).cast::<u8>() = (0i32) as u8;
            let vec3 = (e).into_boxed_slice();
            let ptr3 = vec3.as_ptr().cast::<u8>();
            let len3 = vec3.len();
            ::core::mem::forget(vec3);
            *ptr2.add(8).cast::<usize>() = len3;
            *ptr2.add(4).cast::<*mut u8>() = ptr3.cast_mut();
        }
        Err(e) => {
            *ptr2.add(0).cast::<u8>() = (1i32) as u8;
            let vec4 = (e.into_bytes()).into_boxed_slice();
            let ptr4 = vec4.as_ptr().cast::<u8>();
            let len4 = vec4.len();
            ::core::mem::forget(vec4);
            *ptr2.add(8).cast::<usize>() = len4;
            *ptr2.add(4).cast::<*mut u8>() = ptr4.cast_mut();
        }
    };
    ptr2
}
#[doc(hidden)]
#[allow(non_snake_case)]
pub unsafe fn __post_return_run_task<T: Guest>(arg0: *mut u8) {
    let l0 = i32::from(*arg0.add(0).cast::<u8>());
    match l0 {
        0 => {
            let l1 = *arg0.add(4).cast::<*mut u8>();
            let l2 = *arg0.add(8).cast::<usize>();
            let base3 = l1;
            let len3 = l2;
            _rt::cabi_dealloc(base3, len3 * 1, 1);
        }
        _ => {
            let l4 = *arg0.add(4).cast::<*mut u8>();
            let l5 = *arg0.add(8).cast::<usize>();
            _rt::cabi_dealloc(l4, l5, 1);
        }
    }
}
pub trait Guest {
    fn run_task(request: TaskQueueInput) -> Output;
}
#[doc(hidden)]
macro_rules! __export_world_task_queue_cabi {
    ($ty:ident with_types_in $($path_to_types:tt)*) => {
        const _ : () = { #[export_name = "run-task"] unsafe extern "C" fn
        export_run_task(arg0 : i64, arg1 : * mut u8, arg2 : usize,) -> * mut u8 {
        $($path_to_types)*:: _export_run_task_cabi::<$ty > (arg0, arg1, arg2) }
        #[export_name = "cabi_post_run-task"] unsafe extern "C" fn
        _post_return_run_task(arg0 : * mut u8,) { $($path_to_types)*::
        __post_return_run_task::<$ty > (arg0) } };
    };
}
#[doc(hidden)]
pub(crate) use __export_world_task_queue_cabi;
#[repr(align(4))]
struct _RetArea([::core::mem::MaybeUninit<u8>; 12]);
static mut _RET_AREA: _RetArea = _RetArea([::core::mem::MaybeUninit::uninit(); 12]);
#[allow(dead_code)]
pub mod lay3r {
    #[allow(dead_code)]
    pub mod avs {
        #[allow(dead_code, clippy::all)]
        pub mod types {
            #[used]
            #[doc(hidden)]
            static __FORCE_SECTION_REF: fn() =
                super::super::super::__link_custom_section_describing_imports;
            use super::super::super::_rt;
            /// serialized json, avs wasi and lay3r contract must agree on the types
            /// the runner is agnostic to the data format
            pub type SerializedJson = _rt::Vec<u8>;
            #[derive(Clone)]
            pub struct TaskQueueInput {
                pub timestamp: u64,
                pub request: SerializedJson,
            }
            impl ::core::fmt::Debug for TaskQueueInput {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct("TaskQueueInput")
                        .field("timestamp", &self.timestamp)
                        .field("request", &self.request)
                        .finish()
                }
            }
            pub type Error = _rt::String;
            pub type Output = Result<SerializedJson, Error>;
        }
    }
}
#[allow(dead_code)]
pub mod wasi {
    #[allow(dead_code)]
    pub mod clocks {
        #[allow(dead_code, clippy::all)]
        pub mod monotonic_clock {
            #[used]
            #[doc(hidden)]
            static __FORCE_SECTION_REF: fn() =
                super::super::super::__link_custom_section_describing_imports;
            use super::super::super::_rt;
            pub type Pollable = super::super::super::wasi::io::poll::Pollable;
            pub type Instant = u64;
            pub type Duration = u64;
            #[allow(unused_unsafe, clippy::all)]
            pub fn now() -> Instant {
                unsafe {
                    #[cfg(target_arch = "wasm32")]
                    #[link(wasm_import_module = "wasi:clocks/monotonic-clock@0.2.0")]
                    extern "C" {
                        #[link_name = "now"]
                        fn wit_import() -> i64;
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    fn wit_import() -> i64 {
                        unreachable!()
                    }
                    let ret = wit_import();
                    ret as u64
                }
            }
            #[allow(unused_unsafe, clippy::all)]
            pub fn resolution() -> Duration {
                unsafe {
                    #[cfg(target_arch = "wasm32")]
                    #[link(wasm_import_module = "wasi:clocks/monotonic-clock@0.2.0")]
                    extern "C" {
                        #[link_name = "resolution"]
                        fn wit_import() -> i64;
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    fn wit_import() -> i64 {
                        unreachable!()
                    }
                    let ret = wit_import();
                    ret as u64
                }
            }
            #[allow(unused_unsafe, clippy::all)]
            pub fn subscribe_instant(when: Instant) -> Pollable {
                unsafe {
                    #[cfg(target_arch = "wasm32")]
                    #[link(wasm_import_module = "wasi:clocks/monotonic-clock@0.2.0")]
                    extern "C" {
                        #[link_name = "subscribe-instant"]
                        fn wit_import(_: i64) -> i32;
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    fn wit_import(_: i64) -> i32 {
                        unreachable!()
                    }
                    let ret = wit_import(_rt::as_i64(when));
                    super::super::super::wasi::io::poll::Pollable::from_handle(ret as u32)
                }
            }
            #[allow(unused_unsafe, clippy::all)]
            pub fn subscribe_duration(when: Duration) -> Pollable {
                unsafe {
                    #[cfg(target_arch = "wasm32")]
                    #[link(wasm_import_module = "wasi:clocks/monotonic-clock@0.2.0")]
                    extern "C" {
                        #[link_name = "subscribe-duration"]
                        fn wit_import(_: i64) -> i32;
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    fn wit_import(_: i64) -> i32 {
                        unreachable!()
                    }
                    let ret = wit_import(_rt::as_i64(when));
                    super::super::super::wasi::io::poll::Pollable::from_handle(ret as u32)
                }
            }
        }
    }
    #[allow(dead_code)]
    pub mod http {
        #[allow(dead_code, clippy::all)]
        pub mod types {
            #[used]
            #[doc(hidden)]
            static __FORCE_SECTION_REF: fn() =
                super::super::super::__link_custom_section_describing_imports;
            use super::super::super::_rt;
            pub type Duration = super::super::super::wasi::clocks::monotonic_clock::Duration;
            pub type InputStream = super::super::super::wasi::io::streams::InputStream;
            pub type OutputStream = super::super::super::wasi::io::streams::OutputStream;
            pub type IoError = super::super::super::wasi::io::error::Error;
            pub type Pollable = super::super::super::wasi::io::poll::Pollable;
            #[derive(Clone)]
            pub enum Method {
                Get,
                Head,
                Post,
                Put,
                Delete,
                Connect,
                Options,
                Trace,
                Patch,
                Other(_rt::String),
            }
            impl ::core::fmt::Debug for Method {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    match self {
                        Method::Get => f.debug_tuple("Method::Get").finish(),
                        Method::Head => f.debug_tuple("Method::Head").finish(),
                        Method::Post => f.debug_tuple("Method::Post").finish(),
                        Method::Put => f.debug_tuple("Method::Put").finish(),
                        Method::Delete => f.debug_tuple("Method::Delete").finish(),
                        Method::Connect => f.debug_tuple("Method::Connect").finish(),
                        Method::Options => f.debug_tuple("Method::Options").finish(),
                        Method::Trace => f.debug_tuple("Method::Trace").finish(),
                        Method::Patch => f.debug_tuple("Method::Patch").finish(),
                        Method::Other(e) => f.debug_tuple("Method::Other").field(e).finish(),
                    }
                }
            }
            #[derive(Clone)]
            pub enum Scheme {
                Http,
                Https,
                Other(_rt::String),
            }
            impl ::core::fmt::Debug for Scheme {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    match self {
                        Scheme::Http => f.debug_tuple("Scheme::Http").finish(),
                        Scheme::Https => f.debug_tuple("Scheme::Https").finish(),
                        Scheme::Other(e) => f.debug_tuple("Scheme::Other").field(e).finish(),
                    }
                }
            }
            #[derive(Clone)]
            pub struct DnsErrorPayload {
                pub rcode: Option<_rt::String>,
                pub info_code: Option<u16>,
            }
            impl ::core::fmt::Debug for DnsErrorPayload {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct("DnsErrorPayload")
                        .field("rcode", &self.rcode)
                        .field("info-code", &self.info_code)
                        .finish()
                }
            }
            #[derive(Clone)]
            pub struct TlsAlertReceivedPayload {
                pub alert_id: Option<u8>,
                pub alert_message: Option<_rt::String>,
            }
            impl ::core::fmt::Debug for TlsAlertReceivedPayload {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct("TlsAlertReceivedPayload")
                        .field("alert-id", &self.alert_id)
                        .field("alert-message", &self.alert_message)
                        .finish()
                }
            }
            #[derive(Clone)]
            pub struct FieldSizePayload {
                pub field_name: Option<_rt::String>,
                pub field_size: Option<u32>,
            }
            impl ::core::fmt::Debug for FieldSizePayload {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct("FieldSizePayload")
                        .field("field-name", &self.field_name)
                        .field("field-size", &self.field_size)
                        .finish()
                }
            }
            #[derive(Clone)]
            pub enum ErrorCode {
                DnsTimeout,
                DnsError(DnsErrorPayload),
                DestinationNotFound,
                DestinationUnavailable,
                DestinationIpProhibited,
                DestinationIpUnroutable,
                ConnectionRefused,
                ConnectionTerminated,
                ConnectionTimeout,
                ConnectionReadTimeout,
                ConnectionWriteTimeout,
                ConnectionLimitReached,
                TlsProtocolError,
                TlsCertificateError,
                TlsAlertReceived(TlsAlertReceivedPayload),
                HttpRequestDenied,
                HttpRequestLengthRequired,
                HttpRequestBodySize(Option<u64>),
                HttpRequestMethodInvalid,
                HttpRequestUriInvalid,
                HttpRequestUriTooLong,
                HttpRequestHeaderSectionSize(Option<u32>),
                HttpRequestHeaderSize(Option<FieldSizePayload>),
                HttpRequestTrailerSectionSize(Option<u32>),
                HttpRequestTrailerSize(FieldSizePayload),
                HttpResponseIncomplete,
                HttpResponseHeaderSectionSize(Option<u32>),
                HttpResponseHeaderSize(FieldSizePayload),
                HttpResponseBodySize(Option<u64>),
                HttpResponseTrailerSectionSize(Option<u32>),
                HttpResponseTrailerSize(FieldSizePayload),
                HttpResponseTransferCoding(Option<_rt::String>),
                HttpResponseContentCoding(Option<_rt::String>),
                HttpResponseTimeout,
                HttpUpgradeFailed,
                HttpProtocolError,
                LoopDetected,
                ConfigurationError,
                InternalError(Option<_rt::String>),
            }
            impl ::core::fmt::Debug for ErrorCode {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    match self {
                        ErrorCode::DnsTimeout => f.debug_tuple("ErrorCode::DnsTimeout").finish(),
                        ErrorCode::DnsError(e) => {
                            f.debug_tuple("ErrorCode::DnsError").field(e).finish()
                        }
                        ErrorCode::DestinationNotFound => {
                            f.debug_tuple("ErrorCode::DestinationNotFound").finish()
                        }
                        ErrorCode::DestinationUnavailable => {
                            f.debug_tuple("ErrorCode::DestinationUnavailable").finish()
                        }
                        ErrorCode::DestinationIpProhibited => {
                            f.debug_tuple("ErrorCode::DestinationIpProhibited").finish()
                        }
                        ErrorCode::DestinationIpUnroutable => {
                            f.debug_tuple("ErrorCode::DestinationIpUnroutable").finish()
                        }
                        ErrorCode::ConnectionRefused => {
                            f.debug_tuple("ErrorCode::ConnectionRefused").finish()
                        }
                        ErrorCode::ConnectionTerminated => {
                            f.debug_tuple("ErrorCode::ConnectionTerminated").finish()
                        }
                        ErrorCode::ConnectionTimeout => {
                            f.debug_tuple("ErrorCode::ConnectionTimeout").finish()
                        }
                        ErrorCode::ConnectionReadTimeout => {
                            f.debug_tuple("ErrorCode::ConnectionReadTimeout").finish()
                        }
                        ErrorCode::ConnectionWriteTimeout => {
                            f.debug_tuple("ErrorCode::ConnectionWriteTimeout").finish()
                        }
                        ErrorCode::ConnectionLimitReached => {
                            f.debug_tuple("ErrorCode::ConnectionLimitReached").finish()
                        }
                        ErrorCode::TlsProtocolError => {
                            f.debug_tuple("ErrorCode::TlsProtocolError").finish()
                        }
                        ErrorCode::TlsCertificateError => {
                            f.debug_tuple("ErrorCode::TlsCertificateError").finish()
                        }
                        ErrorCode::TlsAlertReceived(e) => f
                            .debug_tuple("ErrorCode::TlsAlertReceived")
                            .field(e)
                            .finish(),
                        ErrorCode::HttpRequestDenied => {
                            f.debug_tuple("ErrorCode::HttpRequestDenied").finish()
                        }
                        ErrorCode::HttpRequestLengthRequired => f
                            .debug_tuple("ErrorCode::HttpRequestLengthRequired")
                            .finish(),
                        ErrorCode::HttpRequestBodySize(e) => f
                            .debug_tuple("ErrorCode::HttpRequestBodySize")
                            .field(e)
                            .finish(),
                        ErrorCode::HttpRequestMethodInvalid => f
                            .debug_tuple("ErrorCode::HttpRequestMethodInvalid")
                            .finish(),
                        ErrorCode::HttpRequestUriInvalid => {
                            f.debug_tuple("ErrorCode::HttpRequestUriInvalid").finish()
                        }
                        ErrorCode::HttpRequestUriTooLong => {
                            f.debug_tuple("ErrorCode::HttpRequestUriTooLong").finish()
                        }
                        ErrorCode::HttpRequestHeaderSectionSize(e) => f
                            .debug_tuple("ErrorCode::HttpRequestHeaderSectionSize")
                            .field(e)
                            .finish(),
                        ErrorCode::HttpRequestHeaderSize(e) => f
                            .debug_tuple("ErrorCode::HttpRequestHeaderSize")
                            .field(e)
                            .finish(),
                        ErrorCode::HttpRequestTrailerSectionSize(e) => f
                            .debug_tuple("ErrorCode::HttpRequestTrailerSectionSize")
                            .field(e)
                            .finish(),
                        ErrorCode::HttpRequestTrailerSize(e) => f
                            .debug_tuple("ErrorCode::HttpRequestTrailerSize")
                            .field(e)
                            .finish(),
                        ErrorCode::HttpResponseIncomplete => {
                            f.debug_tuple("ErrorCode::HttpResponseIncomplete").finish()
                        }
                        ErrorCode::HttpResponseHeaderSectionSize(e) => f
                            .debug_tuple("ErrorCode::HttpResponseHeaderSectionSize")
                            .field(e)
                            .finish(),
                        ErrorCode::HttpResponseHeaderSize(e) => f
                            .debug_tuple("ErrorCode::HttpResponseHeaderSize")
                            .field(e)
                            .finish(),
                        ErrorCode::HttpResponseBodySize(e) => f
                            .debug_tuple("ErrorCode::HttpResponseBodySize")
                            .field(e)
                            .finish(),
                        ErrorCode::HttpResponseTrailerSectionSize(e) => f
                            .debug_tuple("ErrorCode::HttpResponseTrailerSectionSize")
                            .field(e)
                            .finish(),
                        ErrorCode::HttpResponseTrailerSize(e) => f
                            .debug_tuple("ErrorCode::HttpResponseTrailerSize")
                            .field(e)
                            .finish(),
                        ErrorCode::HttpResponseTransferCoding(e) => f
                            .debug_tuple("ErrorCode::HttpResponseTransferCoding")
                            .field(e)
                            .finish(),
                        ErrorCode::HttpResponseContentCoding(e) => f
                            .debug_tuple("ErrorCode::HttpResponseContentCoding")
                            .field(e)
                            .finish(),
                        ErrorCode::HttpResponseTimeout => {
                            f.debug_tuple("ErrorCode::HttpResponseTimeout").finish()
                        }
                        ErrorCode::HttpUpgradeFailed => {
                            f.debug_tuple("ErrorCode::HttpUpgradeFailed").finish()
                        }
                        ErrorCode::HttpProtocolError => {
                            f.debug_tuple("ErrorCode::HttpProtocolError").finish()
                        }
                        ErrorCode::LoopDetected => {
                            f.debug_tuple("ErrorCode::LoopDetected").finish()
                        }
                        ErrorCode::ConfigurationError => {
                            f.debug_tuple("ErrorCode::ConfigurationError").finish()
                        }
                        ErrorCode::InternalError(e) => {
                            f.debug_tuple("ErrorCode::InternalError").field(e).finish()
                        }
                    }
                }
            }
            impl ::core::fmt::Display for ErrorCode {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    write!(f, "{:?}", self)
                }
            }
            impl std::error::Error for ErrorCode {}
            #[derive(Clone, Copy)]
            pub enum HeaderError {
                InvalidSyntax,
                Forbidden,
                Immutable,
            }
            impl ::core::fmt::Debug for HeaderError {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    match self {
                        HeaderError::InvalidSyntax => {
                            f.debug_tuple("HeaderError::InvalidSyntax").finish()
                        }
                        HeaderError::Forbidden => f.debug_tuple("HeaderError::Forbidden").finish(),
                        HeaderError::Immutable => f.debug_tuple("HeaderError::Immutable").finish(),
                    }
                }
            }
            impl ::core::fmt::Display for HeaderError {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    write!(f, "{:?}", self)
                }
            }
            impl std::error::Error for HeaderError {}
            pub type FieldKey = _rt::String;
            pub type FieldValue = _rt::Vec<u8>;
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct Fields {
                handle: _rt::Resource<Fields>,
            }
            impl Fields {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for Fields {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]fields"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            pub type Headers = Fields;
            pub type Trailers = Fields;
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct IncomingRequest {
                handle: _rt::Resource<IncomingRequest>,
            }
            impl IncomingRequest {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for IncomingRequest {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]incoming-request"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct OutgoingRequest {
                handle: _rt::Resource<OutgoingRequest>,
            }
            impl OutgoingRequest {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for OutgoingRequest {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]outgoing-request"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct RequestOptions {
                handle: _rt::Resource<RequestOptions>,
            }
            impl RequestOptions {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for RequestOptions {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]request-options"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct ResponseOutparam {
                handle: _rt::Resource<ResponseOutparam>,
            }
            impl ResponseOutparam {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for ResponseOutparam {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]response-outparam"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            pub type StatusCode = u16;
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct IncomingResponse {
                handle: _rt::Resource<IncomingResponse>,
            }
            impl IncomingResponse {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for IncomingResponse {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]incoming-response"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct IncomingBody {
                handle: _rt::Resource<IncomingBody>,
            }
            impl IncomingBody {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for IncomingBody {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]incoming-body"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct FutureTrailers {
                handle: _rt::Resource<FutureTrailers>,
            }
            impl FutureTrailers {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for FutureTrailers {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]future-trailers"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct OutgoingResponse {
                handle: _rt::Resource<OutgoingResponse>,
            }
            impl OutgoingResponse {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for OutgoingResponse {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]outgoing-response"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct OutgoingBody {
                handle: _rt::Resource<OutgoingBody>,
            }
            impl OutgoingBody {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for OutgoingBody {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]outgoing-body"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct FutureIncomingResponse {
                handle: _rt::Resource<FutureIncomingResponse>,
            }
            impl FutureIncomingResponse {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for FutureIncomingResponse {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]future-incoming-response"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            impl Fields {
                #[allow(unused_unsafe, clippy::all)]
                pub fn new() -> Self {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[constructor]fields"]
                            fn wit_import() -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import() -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import();
                        Fields::from_handle(ret as u32)
                    }
                }
            }
            impl Fields {
                #[allow(unused_unsafe, clippy::all)]
                pub fn from_list(
                    entries: &[(FieldKey, FieldValue)],
                ) -> Result<Fields, HeaderError> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 8]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 8]);
                        let vec3 = entries;
                        let len3 = vec3.len();
                        let layout3 =
                            _rt::alloc::Layout::from_size_align_unchecked(vec3.len() * 16, 4);
                        let result3 = if layout3.size() != 0 {
                            let ptr = _rt::alloc::alloc(layout3).cast::<u8>();
                            if ptr.is_null() {
                                _rt::alloc::handle_alloc_error(layout3);
                            }
                            ptr
                        } else {
                            {
                                ::core::ptr::null_mut()
                            }
                        };
                        for (i, e) in vec3.into_iter().enumerate() {
                            let base = result3.add(i * 16);
                            {
                                let (t0_0, t0_1) = e;
                                let vec1 = t0_0;
                                let ptr1 = vec1.as_ptr().cast::<u8>();
                                let len1 = vec1.len();
                                *base.add(4).cast::<usize>() = len1;
                                *base.add(0).cast::<*mut u8>() = ptr1.cast_mut();
                                let vec2 = t0_1;
                                let ptr2 = vec2.as_ptr().cast::<u8>();
                                let len2 = vec2.len();
                                *base.add(12).cast::<usize>() = len2;
                                *base.add(8).cast::<*mut u8>() = ptr2.cast_mut();
                            }
                        }
                        let ptr4 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[static]fields.from-list"]
                            fn wit_import(_: *mut u8, _: usize, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: *mut u8, _: usize, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import(result3, len3, ptr4);
                        let l5 = i32::from(*ptr4.add(0).cast::<u8>());
                        if layout3.size() != 0 {
                            _rt::alloc::dealloc(result3.cast(), layout3);
                        }
                        match l5 {
                            0 => {
                                let e = {
                                    let l6 = *ptr4.add(4).cast::<i32>();
                                    Fields::from_handle(l6 as u32)
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l7 = i32::from(*ptr4.add(4).cast::<u8>());
                                    let v8 = match l7 {
                                        0 => HeaderError::InvalidSyntax,
                                        1 => HeaderError::Forbidden,
                                        n => {
                                            debug_assert_eq!(n, 2, "invalid enum discriminant");
                                            HeaderError::Immutable
                                        }
                                    };
                                    v8
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl Fields {
                #[allow(unused_unsafe, clippy::all)]
                pub fn get(&self, name: &FieldKey) -> _rt::Vec<FieldValue> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 8]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 8]);
                        let vec0 = name;
                        let ptr0 = vec0.as_ptr().cast::<u8>();
                        let len0 = vec0.len();
                        let ptr1 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]fields.get"]
                            fn wit_import(_: i32, _: *mut u8, _: usize, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8, _: usize, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0.cast_mut(), len0, ptr1);
                        let l2 = *ptr1.add(0).cast::<*mut u8>();
                        let l3 = *ptr1.add(4).cast::<usize>();
                        let base7 = l2;
                        let len7 = l3;
                        let mut result7 = _rt::Vec::with_capacity(len7);
                        for i in 0..len7 {
                            let base = base7.add(i * 8);
                            let e7 = {
                                let l4 = *base.add(0).cast::<*mut u8>();
                                let l5 = *base.add(4).cast::<usize>();
                                let len6 = l5;
                                _rt::Vec::from_raw_parts(l4.cast(), len6, len6)
                            };
                            result7.push(e7);
                        }
                        _rt::cabi_dealloc(base7, len7 * 8, 4);
                        result7
                    }
                }
            }
            impl Fields {
                #[allow(unused_unsafe, clippy::all)]
                pub fn has(&self, name: &FieldKey) -> bool {
                    unsafe {
                        let vec0 = name;
                        let ptr0 = vec0.as_ptr().cast::<u8>();
                        let len0 = vec0.len();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]fields.has"]
                            fn wit_import(_: i32, _: *mut u8, _: usize) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8, _: usize) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32, ptr0.cast_mut(), len0);
                        _rt::bool_lift(ret as u8)
                    }
                }
            }
            impl Fields {
                #[allow(unused_unsafe, clippy::all)]
                pub fn set(
                    &self,
                    name: &FieldKey,
                    value: &[FieldValue],
                ) -> Result<(), HeaderError> {
                    unsafe {
                        #[repr(align(1))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 2]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 2]);
                        let vec0 = name;
                        let ptr0 = vec0.as_ptr().cast::<u8>();
                        let len0 = vec0.len();
                        let vec2 = value;
                        let len2 = vec2.len();
                        let layout2 =
                            _rt::alloc::Layout::from_size_align_unchecked(vec2.len() * 8, 4);
                        let result2 = if layout2.size() != 0 {
                            let ptr = _rt::alloc::alloc(layout2).cast::<u8>();
                            if ptr.is_null() {
                                _rt::alloc::handle_alloc_error(layout2);
                            }
                            ptr
                        } else {
                            {
                                ::core::ptr::null_mut()
                            }
                        };
                        for (i, e) in vec2.into_iter().enumerate() {
                            let base = result2.add(i * 8);
                            {
                                let vec1 = e;
                                let ptr1 = vec1.as_ptr().cast::<u8>();
                                let len1 = vec1.len();
                                *base.add(4).cast::<usize>() = len1;
                                *base.add(0).cast::<*mut u8>() = ptr1.cast_mut();
                            }
                        }
                        let ptr3 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]fields.set"]
                            fn wit_import(
                                _: i32,
                                _: *mut u8,
                                _: usize,
                                _: *mut u8,
                                _: usize,
                                _: *mut u8,
                            );
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(
                            _: i32,
                            _: *mut u8,
                            _: usize,
                            _: *mut u8,
                            _: usize,
                            _: *mut u8,
                        ) {
                            unreachable!()
                        }
                        wit_import(
                            (self).handle() as i32,
                            ptr0.cast_mut(),
                            len0,
                            result2,
                            len2,
                            ptr3,
                        );
                        let l4 = i32::from(*ptr3.add(0).cast::<u8>());
                        if layout2.size() != 0 {
                            _rt::alloc::dealloc(result2.cast(), layout2);
                        }
                        match l4 {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l5 = i32::from(*ptr3.add(1).cast::<u8>());
                                    let v6 = match l5 {
                                        0 => HeaderError::InvalidSyntax,
                                        1 => HeaderError::Forbidden,
                                        n => {
                                            debug_assert_eq!(n, 2, "invalid enum discriminant");
                                            HeaderError::Immutable
                                        }
                                    };
                                    v6
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl Fields {
                #[allow(unused_unsafe, clippy::all)]
                pub fn delete(&self, name: &FieldKey) -> Result<(), HeaderError> {
                    unsafe {
                        #[repr(align(1))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 2]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 2]);
                        let vec0 = name;
                        let ptr0 = vec0.as_ptr().cast::<u8>();
                        let len0 = vec0.len();
                        let ptr1 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]fields.delete"]
                            fn wit_import(_: i32, _: *mut u8, _: usize, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8, _: usize, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0.cast_mut(), len0, ptr1);
                        let l2 = i32::from(*ptr1.add(0).cast::<u8>());
                        match l2 {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l3 = i32::from(*ptr1.add(1).cast::<u8>());
                                    let v4 = match l3 {
                                        0 => HeaderError::InvalidSyntax,
                                        1 => HeaderError::Forbidden,
                                        n => {
                                            debug_assert_eq!(n, 2, "invalid enum discriminant");
                                            HeaderError::Immutable
                                        }
                                    };
                                    v4
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl Fields {
                #[allow(unused_unsafe, clippy::all)]
                pub fn append(
                    &self,
                    name: &FieldKey,
                    value: &FieldValue,
                ) -> Result<(), HeaderError> {
                    unsafe {
                        #[repr(align(1))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 2]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 2]);
                        let vec0 = name;
                        let ptr0 = vec0.as_ptr().cast::<u8>();
                        let len0 = vec0.len();
                        let vec1 = value;
                        let ptr1 = vec1.as_ptr().cast::<u8>();
                        let len1 = vec1.len();
                        let ptr2 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]fields.append"]
                            fn wit_import(
                                _: i32,
                                _: *mut u8,
                                _: usize,
                                _: *mut u8,
                                _: usize,
                                _: *mut u8,
                            );
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(
                            _: i32,
                            _: *mut u8,
                            _: usize,
                            _: *mut u8,
                            _: usize,
                            _: *mut u8,
                        ) {
                            unreachable!()
                        }
                        wit_import(
                            (self).handle() as i32,
                            ptr0.cast_mut(),
                            len0,
                            ptr1.cast_mut(),
                            len1,
                            ptr2,
                        );
                        let l3 = i32::from(*ptr2.add(0).cast::<u8>());
                        match l3 {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l4 = i32::from(*ptr2.add(1).cast::<u8>());
                                    let v5 = match l4 {
                                        0 => HeaderError::InvalidSyntax,
                                        1 => HeaderError::Forbidden,
                                        n => {
                                            debug_assert_eq!(n, 2, "invalid enum discriminant");
                                            HeaderError::Immutable
                                        }
                                    };
                                    v5
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl Fields {
                #[allow(unused_unsafe, clippy::all)]
                pub fn entries(&self) -> _rt::Vec<(FieldKey, FieldValue)> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 8]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 8]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]fields.entries"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = *ptr0.add(0).cast::<*mut u8>();
                        let l2 = *ptr0.add(4).cast::<usize>();
                        let base9 = l1;
                        let len9 = l2;
                        let mut result9 = _rt::Vec::with_capacity(len9);
                        for i in 0..len9 {
                            let base = base9.add(i * 16);
                            let e9 = {
                                let l3 = *base.add(0).cast::<*mut u8>();
                                let l4 = *base.add(4).cast::<usize>();
                                let len5 = l4;
                                let bytes5 = _rt::Vec::from_raw_parts(l3.cast(), len5, len5);
                                let l6 = *base.add(8).cast::<*mut u8>();
                                let l7 = *base.add(12).cast::<usize>();
                                let len8 = l7;
                                (
                                    _rt::string_lift(bytes5),
                                    _rt::Vec::from_raw_parts(l6.cast(), len8, len8),
                                )
                            };
                            result9.push(e9);
                        }
                        _rt::cabi_dealloc(base9, len9 * 16, 4);
                        result9
                    }
                }
            }
            impl Fields {
                #[allow(unused_unsafe, clippy::all)]
                pub fn clone(&self) -> Fields {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]fields.clone"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32);
                        Fields::from_handle(ret as u32)
                    }
                }
            }
            impl IncomingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn method(&self) -> Method {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]incoming-request.method"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        let v5 = match l1 {
                            0 => Method::Get,
                            1 => Method::Head,
                            2 => Method::Post,
                            3 => Method::Put,
                            4 => Method::Delete,
                            5 => Method::Connect,
                            6 => Method::Options,
                            7 => Method::Trace,
                            8 => Method::Patch,
                            n => {
                                debug_assert_eq!(n, 9, "invalid enum discriminant");
                                let e5 = {
                                    let l2 = *ptr0.add(4).cast::<*mut u8>();
                                    let l3 = *ptr0.add(8).cast::<usize>();
                                    let len4 = l3;
                                    let bytes4 = _rt::Vec::from_raw_parts(l2.cast(), len4, len4);
                                    _rt::string_lift(bytes4)
                                };
                                Method::Other(e5)
                            }
                        };
                        v5
                    }
                }
            }
            impl IncomingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn path_with_query(&self) -> Option<_rt::String> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]incoming-request.path-with-query"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => None,
                            1 => {
                                let e = {
                                    let l2 = *ptr0.add(4).cast::<*mut u8>();
                                    let l3 = *ptr0.add(8).cast::<usize>();
                                    let len4 = l3;
                                    let bytes4 = _rt::Vec::from_raw_parts(l2.cast(), len4, len4);
                                    _rt::string_lift(bytes4)
                                };
                                Some(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl IncomingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn scheme(&self) -> Option<Scheme> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 16]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 16]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]incoming-request.scheme"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => None,
                            1 => {
                                let e = {
                                    let l2 = i32::from(*ptr0.add(4).cast::<u8>());
                                    let v6 = match l2 {
                                        0 => Scheme::Http,
                                        1 => Scheme::Https,
                                        n => {
                                            debug_assert_eq!(n, 2, "invalid enum discriminant");
                                            let e6 = {
                                                let l3 = *ptr0.add(8).cast::<*mut u8>();
                                                let l4 = *ptr0.add(12).cast::<usize>();
                                                let len5 = l4;
                                                let bytes5 =
                                                    _rt::Vec::from_raw_parts(l3.cast(), len5, len5);
                                                _rt::string_lift(bytes5)
                                            };
                                            Scheme::Other(e6)
                                        }
                                    };
                                    v6
                                };
                                Some(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl IncomingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn authority(&self) -> Option<_rt::String> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]incoming-request.authority"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => None,
                            1 => {
                                let e = {
                                    let l2 = *ptr0.add(4).cast::<*mut u8>();
                                    let l3 = *ptr0.add(8).cast::<usize>();
                                    let len4 = l3;
                                    let bytes4 = _rt::Vec::from_raw_parts(l2.cast(), len4, len4);
                                    _rt::string_lift(bytes4)
                                };
                                Some(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl IncomingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn headers(&self) -> Headers {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]incoming-request.headers"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32);
                        Fields::from_handle(ret as u32)
                    }
                }
            }
            impl IncomingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn consume(&self) -> Result<IncomingBody, ()> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 8]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 8]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]incoming-request.consume"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = {
                                    let l2 = *ptr0.add(4).cast::<i32>();
                                    IncomingBody::from_handle(l2 as u32)
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutgoingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn new(headers: Headers) -> Self {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[constructor]outgoing-request"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((&headers).take_handle() as i32);
                        OutgoingRequest::from_handle(ret as u32)
                    }
                }
            }
            impl OutgoingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn body(&self) -> Result<OutgoingBody, ()> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 8]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 8]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-request.body"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = {
                                    let l2 = *ptr0.add(4).cast::<i32>();
                                    OutgoingBody::from_handle(l2 as u32)
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutgoingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn method(&self) -> Method {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-request.method"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        let v5 = match l1 {
                            0 => Method::Get,
                            1 => Method::Head,
                            2 => Method::Post,
                            3 => Method::Put,
                            4 => Method::Delete,
                            5 => Method::Connect,
                            6 => Method::Options,
                            7 => Method::Trace,
                            8 => Method::Patch,
                            n => {
                                debug_assert_eq!(n, 9, "invalid enum discriminant");
                                let e5 = {
                                    let l2 = *ptr0.add(4).cast::<*mut u8>();
                                    let l3 = *ptr0.add(8).cast::<usize>();
                                    let len4 = l3;
                                    let bytes4 = _rt::Vec::from_raw_parts(l2.cast(), len4, len4);
                                    _rt::string_lift(bytes4)
                                };
                                Method::Other(e5)
                            }
                        };
                        v5
                    }
                }
            }
            impl OutgoingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn set_method(&self, method: &Method) -> Result<(), ()> {
                    unsafe {
                        let (result1_0, result1_1, result1_2) = match method {
                            Method::Get => (0i32, ::core::ptr::null_mut(), 0usize),
                            Method::Head => (1i32, ::core::ptr::null_mut(), 0usize),
                            Method::Post => (2i32, ::core::ptr::null_mut(), 0usize),
                            Method::Put => (3i32, ::core::ptr::null_mut(), 0usize),
                            Method::Delete => (4i32, ::core::ptr::null_mut(), 0usize),
                            Method::Connect => (5i32, ::core::ptr::null_mut(), 0usize),
                            Method::Options => (6i32, ::core::ptr::null_mut(), 0usize),
                            Method::Trace => (7i32, ::core::ptr::null_mut(), 0usize),
                            Method::Patch => (8i32, ::core::ptr::null_mut(), 0usize),
                            Method::Other(e) => {
                                let vec0 = e;
                                let ptr0 = vec0.as_ptr().cast::<u8>();
                                let len0 = vec0.len();
                                (9i32, ptr0.cast_mut(), len0)
                            }
                        };
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-request.set-method"]
                            fn wit_import(_: i32, _: i32, _: *mut u8, _: usize) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i32, _: *mut u8, _: usize) -> i32 {
                            unreachable!()
                        }
                        let ret =
                            wit_import((self).handle() as i32, result1_0, result1_1, result1_2);
                        match ret {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutgoingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn path_with_query(&self) -> Option<_rt::String> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-request.path-with-query"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => None,
                            1 => {
                                let e = {
                                    let l2 = *ptr0.add(4).cast::<*mut u8>();
                                    let l3 = *ptr0.add(8).cast::<usize>();
                                    let len4 = l3;
                                    let bytes4 = _rt::Vec::from_raw_parts(l2.cast(), len4, len4);
                                    _rt::string_lift(bytes4)
                                };
                                Some(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutgoingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn set_path_with_query(&self, path_with_query: Option<&str>) -> Result<(), ()> {
                    unsafe {
                        let (result1_0, result1_1, result1_2) = match path_with_query {
                            Some(e) => {
                                let vec0 = e;
                                let ptr0 = vec0.as_ptr().cast::<u8>();
                                let len0 = vec0.len();
                                (1i32, ptr0.cast_mut(), len0)
                            }
                            None => (0i32, ::core::ptr::null_mut(), 0usize),
                        };
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-request.set-path-with-query"]
                            fn wit_import(_: i32, _: i32, _: *mut u8, _: usize) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i32, _: *mut u8, _: usize) -> i32 {
                            unreachable!()
                        }
                        let ret =
                            wit_import((self).handle() as i32, result1_0, result1_1, result1_2);
                        match ret {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutgoingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn scheme(&self) -> Option<Scheme> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 16]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 16]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-request.scheme"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => None,
                            1 => {
                                let e = {
                                    let l2 = i32::from(*ptr0.add(4).cast::<u8>());
                                    let v6 = match l2 {
                                        0 => Scheme::Http,
                                        1 => Scheme::Https,
                                        n => {
                                            debug_assert_eq!(n, 2, "invalid enum discriminant");
                                            let e6 = {
                                                let l3 = *ptr0.add(8).cast::<*mut u8>();
                                                let l4 = *ptr0.add(12).cast::<usize>();
                                                let len5 = l4;
                                                let bytes5 =
                                                    _rt::Vec::from_raw_parts(l3.cast(), len5, len5);
                                                _rt::string_lift(bytes5)
                                            };
                                            Scheme::Other(e6)
                                        }
                                    };
                                    v6
                                };
                                Some(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutgoingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn set_scheme(&self, scheme: Option<&Scheme>) -> Result<(), ()> {
                    unsafe {
                        let (result2_0, result2_1, result2_2, result2_3) = match scheme {
                            Some(e) => {
                                let (result1_0, result1_1, result1_2) = match e {
                                    Scheme::Http => (0i32, ::core::ptr::null_mut(), 0usize),
                                    Scheme::Https => (1i32, ::core::ptr::null_mut(), 0usize),
                                    Scheme::Other(e) => {
                                        let vec0 = e;
                                        let ptr0 = vec0.as_ptr().cast::<u8>();
                                        let len0 = vec0.len();
                                        (2i32, ptr0.cast_mut(), len0)
                                    }
                                };
                                (1i32, result1_0, result1_1, result1_2)
                            }
                            None => (0i32, 0i32, ::core::ptr::null_mut(), 0usize),
                        };
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-request.set-scheme"]
                            fn wit_import(_: i32, _: i32, _: i32, _: *mut u8, _: usize) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i32, _: i32, _: *mut u8, _: usize) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import(
                            (self).handle() as i32,
                            result2_0,
                            result2_1,
                            result2_2,
                            result2_3,
                        );
                        match ret {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutgoingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn authority(&self) -> Option<_rt::String> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-request.authority"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => None,
                            1 => {
                                let e = {
                                    let l2 = *ptr0.add(4).cast::<*mut u8>();
                                    let l3 = *ptr0.add(8).cast::<usize>();
                                    let len4 = l3;
                                    let bytes4 = _rt::Vec::from_raw_parts(l2.cast(), len4, len4);
                                    _rt::string_lift(bytes4)
                                };
                                Some(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutgoingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn set_authority(&self, authority: Option<&str>) -> Result<(), ()> {
                    unsafe {
                        let (result1_0, result1_1, result1_2) = match authority {
                            Some(e) => {
                                let vec0 = e;
                                let ptr0 = vec0.as_ptr().cast::<u8>();
                                let len0 = vec0.len();
                                (1i32, ptr0.cast_mut(), len0)
                            }
                            None => (0i32, ::core::ptr::null_mut(), 0usize),
                        };
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-request.set-authority"]
                            fn wit_import(_: i32, _: i32, _: *mut u8, _: usize) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i32, _: *mut u8, _: usize) -> i32 {
                            unreachable!()
                        }
                        let ret =
                            wit_import((self).handle() as i32, result1_0, result1_1, result1_2);
                        match ret {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutgoingRequest {
                #[allow(unused_unsafe, clippy::all)]
                pub fn headers(&self) -> Headers {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-request.headers"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32);
                        Fields::from_handle(ret as u32)
                    }
                }
            }
            impl RequestOptions {
                #[allow(unused_unsafe, clippy::all)]
                pub fn new() -> Self {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[constructor]request-options"]
                            fn wit_import() -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import() -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import();
                        RequestOptions::from_handle(ret as u32)
                    }
                }
            }
            impl RequestOptions {
                #[allow(unused_unsafe, clippy::all)]
                pub fn connect_timeout(&self) -> Option<Duration> {
                    unsafe {
                        #[repr(align(8))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 16]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 16]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]request-options.connect-timeout"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => None,
                            1 => {
                                let e = {
                                    let l2 = *ptr0.add(8).cast::<i64>();
                                    l2 as u64
                                };
                                Some(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl RequestOptions {
                #[allow(unused_unsafe, clippy::all)]
                pub fn set_connect_timeout(&self, duration: Option<Duration>) -> Result<(), ()> {
                    unsafe {
                        let (result0_0, result0_1) = match duration {
                            Some(e) => (1i32, _rt::as_i64(e)),
                            None => (0i32, 0i64),
                        };
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]request-options.set-connect-timeout"]
                            fn wit_import(_: i32, _: i32, _: i64) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i32, _: i64) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32, result0_0, result0_1);
                        match ret {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl RequestOptions {
                #[allow(unused_unsafe, clippy::all)]
                pub fn first_byte_timeout(&self) -> Option<Duration> {
                    unsafe {
                        #[repr(align(8))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 16]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 16]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]request-options.first-byte-timeout"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => None,
                            1 => {
                                let e = {
                                    let l2 = *ptr0.add(8).cast::<i64>();
                                    l2 as u64
                                };
                                Some(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl RequestOptions {
                #[allow(unused_unsafe, clippy::all)]
                pub fn set_first_byte_timeout(&self, duration: Option<Duration>) -> Result<(), ()> {
                    unsafe {
                        let (result0_0, result0_1) = match duration {
                            Some(e) => (1i32, _rt::as_i64(e)),
                            None => (0i32, 0i64),
                        };
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]request-options.set-first-byte-timeout"]
                            fn wit_import(_: i32, _: i32, _: i64) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i32, _: i64) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32, result0_0, result0_1);
                        match ret {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl RequestOptions {
                #[allow(unused_unsafe, clippy::all)]
                pub fn between_bytes_timeout(&self) -> Option<Duration> {
                    unsafe {
                        #[repr(align(8))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 16]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 16]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]request-options.between-bytes-timeout"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => None,
                            1 => {
                                let e = {
                                    let l2 = *ptr0.add(8).cast::<i64>();
                                    l2 as u64
                                };
                                Some(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl RequestOptions {
                #[allow(unused_unsafe, clippy::all)]
                pub fn set_between_bytes_timeout(
                    &self,
                    duration: Option<Duration>,
                ) -> Result<(), ()> {
                    unsafe {
                        let (result0_0, result0_1) = match duration {
                            Some(e) => (1i32, _rt::as_i64(e)),
                            None => (0i32, 0i64),
                        };
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]request-options.set-between-bytes-timeout"]
                            fn wit_import(_: i32, _: i32, _: i64) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i32, _: i64) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32, result0_0, result0_1);
                        match ret {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl ResponseOutparam {
                #[allow(unused_unsafe, clippy::all)]
                pub fn set(param: ResponseOutparam, response: Result<OutgoingResponse, ErrorCode>) {
                    unsafe {
                        let (
                            result38_0,
                            result38_1,
                            result38_2,
                            result38_3,
                            result38_4,
                            result38_5,
                            result38_6,
                            result38_7,
                        ) = match &response {
                            Ok(e) => (
                                0i32,
                                (e).take_handle() as i32,
                                0i32,
                                ::core::mem::MaybeUninit::<u64>::zeroed(),
                                ::core::ptr::null_mut(),
                                ::core::ptr::null_mut(),
                                0usize,
                                0i32,
                            ),
                            Err(e) => {
                                let (
                                    result37_0,
                                    result37_1,
                                    result37_2,
                                    result37_3,
                                    result37_4,
                                    result37_5,
                                    result37_6,
                                ) = match e {
                                    ErrorCode::DnsTimeout => (
                                        0i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::DnsError(e) => {
                                        let DnsErrorPayload {
                                            rcode: rcode0,
                                            info_code: info_code0,
                                        } = e;
                                        let (result2_0, result2_1, result2_2) = match rcode0 {
                                            Some(e) => {
                                                let vec1 = e;
                                                let ptr1 = vec1.as_ptr().cast::<u8>();
                                                let len1 = vec1.len();
                                                (1i32, ptr1.cast_mut(), len1)
                                            }
                                            None => (0i32, ::core::ptr::null_mut(), 0usize),
                                        };
                                        let (result3_0, result3_1) = match info_code0 {
                                            Some(e) => (1i32, _rt::as_i32(e)),
                                            None => (0i32, 0i32),
                                        };
                                        (
                                            1i32,
                                            result2_0,
                                            {
                                                let mut t =
                                                    ::core::mem::MaybeUninit::<u64>::uninit();
                                                t.as_mut_ptr().cast::<*mut u8>().write(result2_1);
                                                t
                                            },
                                            result2_2 as *mut u8,
                                            result3_0 as *mut u8,
                                            result3_1 as usize,
                                            0i32,
                                        )
                                    }
                                    ErrorCode::DestinationNotFound => (
                                        2i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::DestinationUnavailable => (
                                        3i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::DestinationIpProhibited => (
                                        4i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::DestinationIpUnroutable => (
                                        5i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::ConnectionRefused => (
                                        6i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::ConnectionTerminated => (
                                        7i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::ConnectionTimeout => (
                                        8i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::ConnectionReadTimeout => (
                                        9i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::ConnectionWriteTimeout => (
                                        10i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::ConnectionLimitReached => (
                                        11i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::TlsProtocolError => (
                                        12i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::TlsCertificateError => (
                                        13i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::TlsAlertReceived(e) => {
                                        let TlsAlertReceivedPayload {
                                            alert_id: alert_id4,
                                            alert_message: alert_message4,
                                        } = e;
                                        let (result5_0, result5_1) = match alert_id4 {
                                            Some(e) => (1i32, _rt::as_i32(e)),
                                            None => (0i32, 0i32),
                                        };
                                        let (result7_0, result7_1, result7_2) = match alert_message4
                                        {
                                            Some(e) => {
                                                let vec6 = e;
                                                let ptr6 = vec6.as_ptr().cast::<u8>();
                                                let len6 = vec6.len();
                                                (1i32, ptr6.cast_mut(), len6)
                                            }
                                            None => (0i32, ::core::ptr::null_mut(), 0usize),
                                        };
                                        (
                                            14i32,
                                            result5_0,
                                            ::core::mem::MaybeUninit::new(
                                                i64::from(result5_1) as u64
                                            ),
                                            result7_0 as *mut u8,
                                            result7_1,
                                            result7_2,
                                            0i32,
                                        )
                                    }
                                    ErrorCode::HttpRequestDenied => (
                                        15i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::HttpRequestLengthRequired => (
                                        16i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::HttpRequestBodySize(e) => {
                                        let (result8_0, result8_1) = match e {
                                            Some(e) => (1i32, _rt::as_i64(e)),
                                            None => (0i32, 0i64),
                                        };
                                        (
                                            17i32,
                                            result8_0,
                                            ::core::mem::MaybeUninit::new(result8_1 as u64),
                                            ::core::ptr::null_mut(),
                                            ::core::ptr::null_mut(),
                                            0usize,
                                            0i32,
                                        )
                                    }
                                    ErrorCode::HttpRequestMethodInvalid => (
                                        18i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::HttpRequestUriInvalid => (
                                        19i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::HttpRequestUriTooLong => (
                                        20i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::HttpRequestHeaderSectionSize(e) => {
                                        let (result9_0, result9_1) = match e {
                                            Some(e) => (1i32, _rt::as_i32(e)),
                                            None => (0i32, 0i32),
                                        };
                                        (
                                            21i32,
                                            result9_0,
                                            ::core::mem::MaybeUninit::new(
                                                i64::from(result9_1) as u64
                                            ),
                                            ::core::ptr::null_mut(),
                                            ::core::ptr::null_mut(),
                                            0usize,
                                            0i32,
                                        )
                                    }
                                    ErrorCode::HttpRequestHeaderSize(e) => {
                                        let (
                                            result14_0,
                                            result14_1,
                                            result14_2,
                                            result14_3,
                                            result14_4,
                                            result14_5,
                                        ) = match e {
                                            Some(e) => {
                                                let FieldSizePayload {
                                                    field_name: field_name10,
                                                    field_size: field_size10,
                                                } = e;
                                                let (result12_0, result12_1, result12_2) =
                                                    match field_name10 {
                                                        Some(e) => {
                                                            let vec11 = e;
                                                            let ptr11 = vec11.as_ptr().cast::<u8>();
                                                            let len11 = vec11.len();
                                                            (1i32, ptr11.cast_mut(), len11)
                                                        }
                                                        None => {
                                                            (0i32, ::core::ptr::null_mut(), 0usize)
                                                        }
                                                    };
                                                let (result13_0, result13_1) = match field_size10 {
                                                    Some(e) => (1i32, _rt::as_i32(e)),
                                                    None => (0i32, 0i32),
                                                };
                                                (
                                                    1i32, result12_0, result12_1, result12_2,
                                                    result13_0, result13_1,
                                                )
                                            }
                                            None => (
                                                0i32,
                                                0i32,
                                                ::core::ptr::null_mut(),
                                                0usize,
                                                0i32,
                                                0i32,
                                            ),
                                        };
                                        (
                                            22i32,
                                            result14_0,
                                            ::core::mem::MaybeUninit::new(
                                                i64::from(result14_1) as u64
                                            ),
                                            result14_2,
                                            result14_3 as *mut u8,
                                            result14_4 as usize,
                                            result14_5,
                                        )
                                    }
                                    ErrorCode::HttpRequestTrailerSectionSize(e) => {
                                        let (result15_0, result15_1) = match e {
                                            Some(e) => (1i32, _rt::as_i32(e)),
                                            None => (0i32, 0i32),
                                        };
                                        (
                                            23i32,
                                            result15_0,
                                            ::core::mem::MaybeUninit::new(
                                                i64::from(result15_1) as u64
                                            ),
                                            ::core::ptr::null_mut(),
                                            ::core::ptr::null_mut(),
                                            0usize,
                                            0i32,
                                        )
                                    }
                                    ErrorCode::HttpRequestTrailerSize(e) => {
                                        let FieldSizePayload {
                                            field_name: field_name16,
                                            field_size: field_size16,
                                        } = e;
                                        let (result18_0, result18_1, result18_2) =
                                            match field_name16 {
                                                Some(e) => {
                                                    let vec17 = e;
                                                    let ptr17 = vec17.as_ptr().cast::<u8>();
                                                    let len17 = vec17.len();
                                                    (1i32, ptr17.cast_mut(), len17)
                                                }
                                                None => (0i32, ::core::ptr::null_mut(), 0usize),
                                            };
                                        let (result19_0, result19_1) = match field_size16 {
                                            Some(e) => (1i32, _rt::as_i32(e)),
                                            None => (0i32, 0i32),
                                        };
                                        (
                                            24i32,
                                            result18_0,
                                            {
                                                let mut t =
                                                    ::core::mem::MaybeUninit::<u64>::uninit();
                                                t.as_mut_ptr().cast::<*mut u8>().write(result18_1);
                                                t
                                            },
                                            result18_2 as *mut u8,
                                            result19_0 as *mut u8,
                                            result19_1 as usize,
                                            0i32,
                                        )
                                    }
                                    ErrorCode::HttpResponseIncomplete => (
                                        25i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::HttpResponseHeaderSectionSize(e) => {
                                        let (result20_0, result20_1) = match e {
                                            Some(e) => (1i32, _rt::as_i32(e)),
                                            None => (0i32, 0i32),
                                        };
                                        (
                                            26i32,
                                            result20_0,
                                            ::core::mem::MaybeUninit::new(
                                                i64::from(result20_1) as u64
                                            ),
                                            ::core::ptr::null_mut(),
                                            ::core::ptr::null_mut(),
                                            0usize,
                                            0i32,
                                        )
                                    }
                                    ErrorCode::HttpResponseHeaderSize(e) => {
                                        let FieldSizePayload {
                                            field_name: field_name21,
                                            field_size: field_size21,
                                        } = e;
                                        let (result23_0, result23_1, result23_2) =
                                            match field_name21 {
                                                Some(e) => {
                                                    let vec22 = e;
                                                    let ptr22 = vec22.as_ptr().cast::<u8>();
                                                    let len22 = vec22.len();
                                                    (1i32, ptr22.cast_mut(), len22)
                                                }
                                                None => (0i32, ::core::ptr::null_mut(), 0usize),
                                            };
                                        let (result24_0, result24_1) = match field_size21 {
                                            Some(e) => (1i32, _rt::as_i32(e)),
                                            None => (0i32, 0i32),
                                        };
                                        (
                                            27i32,
                                            result23_0,
                                            {
                                                let mut t =
                                                    ::core::mem::MaybeUninit::<u64>::uninit();
                                                t.as_mut_ptr().cast::<*mut u8>().write(result23_1);
                                                t
                                            },
                                            result23_2 as *mut u8,
                                            result24_0 as *mut u8,
                                            result24_1 as usize,
                                            0i32,
                                        )
                                    }
                                    ErrorCode::HttpResponseBodySize(e) => {
                                        let (result25_0, result25_1) = match e {
                                            Some(e) => (1i32, _rt::as_i64(e)),
                                            None => (0i32, 0i64),
                                        };
                                        (
                                            28i32,
                                            result25_0,
                                            ::core::mem::MaybeUninit::new(result25_1 as u64),
                                            ::core::ptr::null_mut(),
                                            ::core::ptr::null_mut(),
                                            0usize,
                                            0i32,
                                        )
                                    }
                                    ErrorCode::HttpResponseTrailerSectionSize(e) => {
                                        let (result26_0, result26_1) = match e {
                                            Some(e) => (1i32, _rt::as_i32(e)),
                                            None => (0i32, 0i32),
                                        };
                                        (
                                            29i32,
                                            result26_0,
                                            ::core::mem::MaybeUninit::new(
                                                i64::from(result26_1) as u64
                                            ),
                                            ::core::ptr::null_mut(),
                                            ::core::ptr::null_mut(),
                                            0usize,
                                            0i32,
                                        )
                                    }
                                    ErrorCode::HttpResponseTrailerSize(e) => {
                                        let FieldSizePayload {
                                            field_name: field_name27,
                                            field_size: field_size27,
                                        } = e;
                                        let (result29_0, result29_1, result29_2) =
                                            match field_name27 {
                                                Some(e) => {
                                                    let vec28 = e;
                                                    let ptr28 = vec28.as_ptr().cast::<u8>();
                                                    let len28 = vec28.len();
                                                    (1i32, ptr28.cast_mut(), len28)
                                                }
                                                None => (0i32, ::core::ptr::null_mut(), 0usize),
                                            };
                                        let (result30_0, result30_1) = match field_size27 {
                                            Some(e) => (1i32, _rt::as_i32(e)),
                                            None => (0i32, 0i32),
                                        };
                                        (
                                            30i32,
                                            result29_0,
                                            {
                                                let mut t =
                                                    ::core::mem::MaybeUninit::<u64>::uninit();
                                                t.as_mut_ptr().cast::<*mut u8>().write(result29_1);
                                                t
                                            },
                                            result29_2 as *mut u8,
                                            result30_0 as *mut u8,
                                            result30_1 as usize,
                                            0i32,
                                        )
                                    }
                                    ErrorCode::HttpResponseTransferCoding(e) => {
                                        let (result32_0, result32_1, result32_2) = match e {
                                            Some(e) => {
                                                let vec31 = e;
                                                let ptr31 = vec31.as_ptr().cast::<u8>();
                                                let len31 = vec31.len();
                                                (1i32, ptr31.cast_mut(), len31)
                                            }
                                            None => (0i32, ::core::ptr::null_mut(), 0usize),
                                        };
                                        (
                                            31i32,
                                            result32_0,
                                            {
                                                let mut t =
                                                    ::core::mem::MaybeUninit::<u64>::uninit();
                                                t.as_mut_ptr().cast::<*mut u8>().write(result32_1);
                                                t
                                            },
                                            result32_2 as *mut u8,
                                            ::core::ptr::null_mut(),
                                            0usize,
                                            0i32,
                                        )
                                    }
                                    ErrorCode::HttpResponseContentCoding(e) => {
                                        let (result34_0, result34_1, result34_2) = match e {
                                            Some(e) => {
                                                let vec33 = e;
                                                let ptr33 = vec33.as_ptr().cast::<u8>();
                                                let len33 = vec33.len();
                                                (1i32, ptr33.cast_mut(), len33)
                                            }
                                            None => (0i32, ::core::ptr::null_mut(), 0usize),
                                        };
                                        (
                                            32i32,
                                            result34_0,
                                            {
                                                let mut t =
                                                    ::core::mem::MaybeUninit::<u64>::uninit();
                                                t.as_mut_ptr().cast::<*mut u8>().write(result34_1);
                                                t
                                            },
                                            result34_2 as *mut u8,
                                            ::core::ptr::null_mut(),
                                            0usize,
                                            0i32,
                                        )
                                    }
                                    ErrorCode::HttpResponseTimeout => (
                                        33i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::HttpUpgradeFailed => (
                                        34i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::HttpProtocolError => (
                                        35i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::LoopDetected => (
                                        36i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::ConfigurationError => (
                                        37i32,
                                        0i32,
                                        ::core::mem::MaybeUninit::<u64>::zeroed(),
                                        ::core::ptr::null_mut(),
                                        ::core::ptr::null_mut(),
                                        0usize,
                                        0i32,
                                    ),
                                    ErrorCode::InternalError(e) => {
                                        let (result36_0, result36_1, result36_2) = match e {
                                            Some(e) => {
                                                let vec35 = e;
                                                let ptr35 = vec35.as_ptr().cast::<u8>();
                                                let len35 = vec35.len();
                                                (1i32, ptr35.cast_mut(), len35)
                                            }
                                            None => (0i32, ::core::ptr::null_mut(), 0usize),
                                        };
                                        (
                                            38i32,
                                            result36_0,
                                            {
                                                let mut t =
                                                    ::core::mem::MaybeUninit::<u64>::uninit();
                                                t.as_mut_ptr().cast::<*mut u8>().write(result36_1);
                                                t
                                            },
                                            result36_2 as *mut u8,
                                            ::core::ptr::null_mut(),
                                            0usize,
                                            0i32,
                                        )
                                    }
                                };
                                (
                                    1i32, result37_0, result37_1, result37_2, result37_3,
                                    result37_4, result37_5, result37_6,
                                )
                            }
                        };
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[static]response-outparam.set"]
                            fn wit_import(
                                _: i32,
                                _: i32,
                                _: i32,
                                _: i32,
                                _: ::core::mem::MaybeUninit<u64>,
                                _: *mut u8,
                                _: *mut u8,
                                _: usize,
                                _: i32,
                            );
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(
                            _: i32,
                            _: i32,
                            _: i32,
                            _: i32,
                            _: ::core::mem::MaybeUninit<u64>,
                            _: *mut u8,
                            _: *mut u8,
                            _: usize,
                            _: i32,
                        ) {
                            unreachable!()
                        }
                        wit_import(
                            (&param).take_handle() as i32,
                            result38_0,
                            result38_1,
                            result38_2,
                            result38_3,
                            result38_4,
                            result38_5,
                            result38_6,
                            result38_7,
                        );
                    }
                }
            }
            impl IncomingResponse {
                #[allow(unused_unsafe, clippy::all)]
                pub fn status(&self) -> StatusCode {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]incoming-response.status"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32);
                        ret as u16
                    }
                }
            }
            impl IncomingResponse {
                #[allow(unused_unsafe, clippy::all)]
                pub fn headers(&self) -> Headers {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]incoming-response.headers"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32);
                        Fields::from_handle(ret as u32)
                    }
                }
            }
            impl IncomingResponse {
                #[allow(unused_unsafe, clippy::all)]
                pub fn consume(&self) -> Result<IncomingBody, ()> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 8]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 8]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]incoming-response.consume"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = {
                                    let l2 = *ptr0.add(4).cast::<i32>();
                                    IncomingBody::from_handle(l2 as u32)
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl IncomingBody {
                #[allow(unused_unsafe, clippy::all)]
                pub fn stream(&self) -> Result<InputStream, ()> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 8]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 8]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]incoming-body.stream"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = {
                                    let l2 = *ptr0.add(4).cast::<i32>();
                                    super::super::super::wasi::io::streams::InputStream::from_handle(
                                        l2 as u32,
                                    )
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl IncomingBody {
                #[allow(unused_unsafe, clippy::all)]
                pub fn finish(this: IncomingBody) -> FutureTrailers {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[static]incoming-body.finish"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((&this).take_handle() as i32);
                        FutureTrailers::from_handle(ret as u32)
                    }
                }
            }
            impl FutureTrailers {
                #[allow(unused_unsafe, clippy::all)]
                pub fn subscribe(&self) -> Pollable {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]future-trailers.subscribe"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32);
                        super::super::super::wasi::io::poll::Pollable::from_handle(ret as u32)
                    }
                }
            }
            impl FutureTrailers {
                #[allow(unused_unsafe, clippy::all)]
                pub fn get(&self) -> Option<Result<Result<Option<Trailers>, ErrorCode>, ()>> {
                    unsafe {
                        #[repr(align(8))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 56]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 56]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]future-trailers.get"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => None,
                            1 => {
                                let e = {
                                    let l2 = i32::from(*ptr0.add(8).cast::<u8>());
                                    match l2 {
                                        0 => {
                                            let e = {
                                                let l3 = i32::from(*ptr0.add(16).cast::<u8>());
                                                match l3 {
                                                    0 => {
                                                        let e = {
                                                            let l4 = i32::from(
                                                                *ptr0.add(24).cast::<u8>(),
                                                            );
                                                            match l4 {
                                                                0 => None,
                                                                1 => {
                                                                    let e = {
                                                                        let l5 = *ptr0
                                                                            .add(28)
                                                                            .cast::<i32>();
                                                                        Fields::from_handle(
                                                                            l5 as u32,
                                                                        )
                                                                    };
                                                                    Some(e)
                                                                }
                                                                _ => {
                                                                    _rt::invalid_enum_discriminant()
                                                                }
                                                            }
                                                        };
                                                        Ok(e)
                                                    }
                                                    1 => {
                                                        let e = {
                                                            let l6 = i32::from(
                                                                *ptr0.add(24).cast::<u8>(),
                                                            );
                                                            let v68 = match l6 {
                                                                0 => ErrorCode::DnsTimeout,
                                                                1 => {
                                                                    let e68 = {
                                                                        let l7 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        let l11 = i32::from(*ptr0.add(44).cast::<u8>());
                                                                        DnsErrorPayload {
                                                                            rcode: match l7 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l8 = *ptr0.add(36).cast::<*mut u8>();
                                                                                        let l9 = *ptr0.add(40).cast::<usize>();
                                                                                        let len10 = l9;
                                                                                        let bytes10 = _rt::Vec::from_raw_parts(
                                                                                            l8.cast(),
                                                                                            len10,
                                                                                            len10,
                                                                                        );
                                                                                        _rt::string_lift(bytes10)
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                            info_code: match l11 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l12 = i32::from(*ptr0.add(46).cast::<u16>());
                                                                                        l12 as u16
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                        }
                                                                    };
                                                                    ErrorCode::DnsError(e68)
                                                                }
                                                                2 => ErrorCode::DestinationNotFound,
                                                                3 => ErrorCode::DestinationUnavailable,
                                                                4 => ErrorCode::DestinationIpProhibited,
                                                                5 => ErrorCode::DestinationIpUnroutable,
                                                                6 => ErrorCode::ConnectionRefused,
                                                                7 => ErrorCode::ConnectionTerminated,
                                                                8 => ErrorCode::ConnectionTimeout,
                                                                9 => ErrorCode::ConnectionReadTimeout,
                                                                10 => ErrorCode::ConnectionWriteTimeout,
                                                                11 => ErrorCode::ConnectionLimitReached,
                                                                12 => ErrorCode::TlsProtocolError,
                                                                13 => ErrorCode::TlsCertificateError,
                                                                14 => {
                                                                    let e68 = {
                                                                        let l13 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        let l15 = i32::from(*ptr0.add(36).cast::<u8>());
                                                                        TlsAlertReceivedPayload {
                                                                            alert_id: match l13 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l14 = i32::from(*ptr0.add(33).cast::<u8>());
                                                                                        l14 as u8
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                            alert_message: match l15 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l16 = *ptr0.add(40).cast::<*mut u8>();
                                                                                        let l17 = *ptr0.add(44).cast::<usize>();
                                                                                        let len18 = l17;
                                                                                        let bytes18 = _rt::Vec::from_raw_parts(
                                                                                            l16.cast(),
                                                                                            len18,
                                                                                            len18,
                                                                                        );
                                                                                        _rt::string_lift(bytes18)
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                        }
                                                                    };
                                                                    ErrorCode::TlsAlertReceived(e68)
                                                                }
                                                                15 => ErrorCode::HttpRequestDenied,
                                                                16 => ErrorCode::HttpRequestLengthRequired,
                                                                17 => {
                                                                    let e68 = {
                                                                        let l19 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l19 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l20 = *ptr0.add(40).cast::<i64>();
                                                                                    l20 as u64
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpRequestBodySize(e68)
                                                                }
                                                                18 => ErrorCode::HttpRequestMethodInvalid,
                                                                19 => ErrorCode::HttpRequestUriInvalid,
                                                                20 => ErrorCode::HttpRequestUriTooLong,
                                                                21 => {
                                                                    let e68 = {
                                                                        let l21 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l21 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l22 = *ptr0.add(36).cast::<i32>();
                                                                                    l22 as u32
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpRequestHeaderSectionSize(e68)
                                                                }
                                                                22 => {
                                                                    let e68 = {
                                                                        let l23 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l23 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l24 = i32::from(*ptr0.add(36).cast::<u8>());
                                                                                    let l28 = i32::from(*ptr0.add(48).cast::<u8>());
                                                                                    FieldSizePayload {
                                                                                        field_name: match l24 {
                                                                                            0 => None,
                                                                                            1 => {
                                                                                                let e = {
                                                                                                    let l25 = *ptr0.add(40).cast::<*mut u8>();
                                                                                                    let l26 = *ptr0.add(44).cast::<usize>();
                                                                                                    let len27 = l26;
                                                                                                    let bytes27 = _rt::Vec::from_raw_parts(
                                                                                                        l25.cast(),
                                                                                                        len27,
                                                                                                        len27,
                                                                                                    );
                                                                                                    _rt::string_lift(bytes27)
                                                                                                };
                                                                                                Some(e)
                                                                                            }
                                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                                        },
                                                                                        field_size: match l28 {
                                                                                            0 => None,
                                                                                            1 => {
                                                                                                let e = {
                                                                                                    let l29 = *ptr0.add(52).cast::<i32>();
                                                                                                    l29 as u32
                                                                                                };
                                                                                                Some(e)
                                                                                            }
                                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                                        },
                                                                                    }
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpRequestHeaderSize(e68)
                                                                }
                                                                23 => {
                                                                    let e68 = {
                                                                        let l30 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l30 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l31 = *ptr0.add(36).cast::<i32>();
                                                                                    l31 as u32
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpRequestTrailerSectionSize(e68)
                                                                }
                                                                24 => {
                                                                    let e68 = {
                                                                        let l32 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        let l36 = i32::from(*ptr0.add(44).cast::<u8>());
                                                                        FieldSizePayload {
                                                                            field_name: match l32 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l33 = *ptr0.add(36).cast::<*mut u8>();
                                                                                        let l34 = *ptr0.add(40).cast::<usize>();
                                                                                        let len35 = l34;
                                                                                        let bytes35 = _rt::Vec::from_raw_parts(
                                                                                            l33.cast(),
                                                                                            len35,
                                                                                            len35,
                                                                                        );
                                                                                        _rt::string_lift(bytes35)
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                            field_size: match l36 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l37 = *ptr0.add(48).cast::<i32>();
                                                                                        l37 as u32
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpRequestTrailerSize(e68)
                                                                }
                                                                25 => ErrorCode::HttpResponseIncomplete,
                                                                26 => {
                                                                    let e68 = {
                                                                        let l38 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l38 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l39 = *ptr0.add(36).cast::<i32>();
                                                                                    l39 as u32
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseHeaderSectionSize(e68)
                                                                }
                                                                27 => {
                                                                    let e68 = {
                                                                        let l40 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        let l44 = i32::from(*ptr0.add(44).cast::<u8>());
                                                                        FieldSizePayload {
                                                                            field_name: match l40 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l41 = *ptr0.add(36).cast::<*mut u8>();
                                                                                        let l42 = *ptr0.add(40).cast::<usize>();
                                                                                        let len43 = l42;
                                                                                        let bytes43 = _rt::Vec::from_raw_parts(
                                                                                            l41.cast(),
                                                                                            len43,
                                                                                            len43,
                                                                                        );
                                                                                        _rt::string_lift(bytes43)
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                            field_size: match l44 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l45 = *ptr0.add(48).cast::<i32>();
                                                                                        l45 as u32
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseHeaderSize(e68)
                                                                }
                                                                28 => {
                                                                    let e68 = {
                                                                        let l46 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l46 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l47 = *ptr0.add(40).cast::<i64>();
                                                                                    l47 as u64
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseBodySize(e68)
                                                                }
                                                                29 => {
                                                                    let e68 = {
                                                                        let l48 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l48 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l49 = *ptr0.add(36).cast::<i32>();
                                                                                    l49 as u32
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseTrailerSectionSize(e68)
                                                                }
                                                                30 => {
                                                                    let e68 = {
                                                                        let l50 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        let l54 = i32::from(*ptr0.add(44).cast::<u8>());
                                                                        FieldSizePayload {
                                                                            field_name: match l50 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l51 = *ptr0.add(36).cast::<*mut u8>();
                                                                                        let l52 = *ptr0.add(40).cast::<usize>();
                                                                                        let len53 = l52;
                                                                                        let bytes53 = _rt::Vec::from_raw_parts(
                                                                                            l51.cast(),
                                                                                            len53,
                                                                                            len53,
                                                                                        );
                                                                                        _rt::string_lift(bytes53)
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                            field_size: match l54 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l55 = *ptr0.add(48).cast::<i32>();
                                                                                        l55 as u32
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseTrailerSize(e68)
                                                                }
                                                                31 => {
                                                                    let e68 = {
                                                                        let l56 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l56 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l57 = *ptr0.add(36).cast::<*mut u8>();
                                                                                    let l58 = *ptr0.add(40).cast::<usize>();
                                                                                    let len59 = l58;
                                                                                    let bytes59 = _rt::Vec::from_raw_parts(
                                                                                        l57.cast(),
                                                                                        len59,
                                                                                        len59,
                                                                                    );
                                                                                    _rt::string_lift(bytes59)
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseTransferCoding(e68)
                                                                }
                                                                32 => {
                                                                    let e68 = {
                                                                        let l60 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l60 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l61 = *ptr0.add(36).cast::<*mut u8>();
                                                                                    let l62 = *ptr0.add(40).cast::<usize>();
                                                                                    let len63 = l62;
                                                                                    let bytes63 = _rt::Vec::from_raw_parts(
                                                                                        l61.cast(),
                                                                                        len63,
                                                                                        len63,
                                                                                    );
                                                                                    _rt::string_lift(bytes63)
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseContentCoding(e68)
                                                                }
                                                                33 => ErrorCode::HttpResponseTimeout,
                                                                34 => ErrorCode::HttpUpgradeFailed,
                                                                35 => ErrorCode::HttpProtocolError,
                                                                36 => ErrorCode::LoopDetected,
                                                                37 => ErrorCode::ConfigurationError,
                                                                n => {
                                                                    debug_assert_eq!(n, 38, "invalid enum discriminant");
                                                                    let e68 = {
                                                                        let l64 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l64 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l65 = *ptr0.add(36).cast::<*mut u8>();
                                                                                    let l66 = *ptr0.add(40).cast::<usize>();
                                                                                    let len67 = l66;
                                                                                    let bytes67 = _rt::Vec::from_raw_parts(
                                                                                        l65.cast(),
                                                                                        len67,
                                                                                        len67,
                                                                                    );
                                                                                    _rt::string_lift(bytes67)
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::InternalError(e68)
                                                                }
                                                            };
                                                            v68
                                                        };
                                                        Err(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                }
                                            };
                                            Ok(e)
                                        }
                                        1 => {
                                            let e = ();
                                            Err(e)
                                        }
                                        _ => _rt::invalid_enum_discriminant(),
                                    }
                                };
                                Some(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutgoingResponse {
                #[allow(unused_unsafe, clippy::all)]
                pub fn new(headers: Headers) -> Self {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[constructor]outgoing-response"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((&headers).take_handle() as i32);
                        OutgoingResponse::from_handle(ret as u32)
                    }
                }
            }
            impl OutgoingResponse {
                #[allow(unused_unsafe, clippy::all)]
                pub fn status_code(&self) -> StatusCode {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-response.status-code"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32);
                        ret as u16
                    }
                }
            }
            impl OutgoingResponse {
                #[allow(unused_unsafe, clippy::all)]
                pub fn set_status_code(&self, status_code: StatusCode) -> Result<(), ()> {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-response.set-status-code"]
                            fn wit_import(_: i32, _: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32, _rt::as_i32(status_code));
                        match ret {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutgoingResponse {
                #[allow(unused_unsafe, clippy::all)]
                pub fn headers(&self) -> Headers {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-response.headers"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32);
                        Fields::from_handle(ret as u32)
                    }
                }
            }
            impl OutgoingResponse {
                #[allow(unused_unsafe, clippy::all)]
                pub fn body(&self) -> Result<OutgoingBody, ()> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 8]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 8]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-response.body"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = {
                                    let l2 = *ptr0.add(4).cast::<i32>();
                                    OutgoingBody::from_handle(l2 as u32)
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutgoingBody {
                #[allow(unused_unsafe, clippy::all)]
                pub fn write(&self) -> Result<OutputStream, ()> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 8]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 8]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]outgoing-body.write"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = {
                                    let l2 = *ptr0.add(4).cast::<i32>();
                                    super::super::super::wasi::io::streams::OutputStream::from_handle(
                                        l2 as u32,
                                    )
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = ();
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutgoingBody {
                #[allow(unused_unsafe, clippy::all)]
                pub fn finish(
                    this: OutgoingBody,
                    trailers: Option<Trailers>,
                ) -> Result<(), ErrorCode> {
                    unsafe {
                        #[repr(align(8))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 40]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 40]);
                        let (result0_0, result0_1) = match &trailers {
                            Some(e) => (1i32, (e).take_handle() as i32),
                            None => (0i32, 0i32),
                        };
                        let ptr1 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[static]outgoing-body.finish"]
                            fn wit_import(_: i32, _: i32, _: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i32, _: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((&this).take_handle() as i32, result0_0, result0_1, ptr1);
                        let l2 = i32::from(*ptr1.add(0).cast::<u8>());
                        match l2 {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l3 = i32::from(*ptr1.add(8).cast::<u8>());
                                    let v65 = match l3 {
                                        0 => ErrorCode::DnsTimeout,
                                        1 => {
                                            let e65 = {
                                                let l4 = i32::from(*ptr1.add(16).cast::<u8>());
                                                let l8 = i32::from(*ptr1.add(28).cast::<u8>());
                                                DnsErrorPayload {
                                                    rcode: match l4 {
                                                        0 => None,
                                                        1 => {
                                                            let e = {
                                                                let l5 =
                                                                    *ptr1.add(20).cast::<*mut u8>();
                                                                let l6 =
                                                                    *ptr1.add(24).cast::<usize>();
                                                                let len7 = l6;
                                                                let bytes7 =
                                                                    _rt::Vec::from_raw_parts(
                                                                        l5.cast(),
                                                                        len7,
                                                                        len7,
                                                                    );
                                                                _rt::string_lift(bytes7)
                                                            };
                                                            Some(e)
                                                        }
                                                        _ => _rt::invalid_enum_discriminant(),
                                                    },
                                                    info_code: match l8 {
                                                        0 => None,
                                                        1 => {
                                                            let e = {
                                                                let l9 = i32::from(
                                                                    *ptr1.add(30).cast::<u16>(),
                                                                );
                                                                l9 as u16
                                                            };
                                                            Some(e)
                                                        }
                                                        _ => _rt::invalid_enum_discriminant(),
                                                    },
                                                }
                                            };
                                            ErrorCode::DnsError(e65)
                                        }
                                        2 => ErrorCode::DestinationNotFound,
                                        3 => ErrorCode::DestinationUnavailable,
                                        4 => ErrorCode::DestinationIpProhibited,
                                        5 => ErrorCode::DestinationIpUnroutable,
                                        6 => ErrorCode::ConnectionRefused,
                                        7 => ErrorCode::ConnectionTerminated,
                                        8 => ErrorCode::ConnectionTimeout,
                                        9 => ErrorCode::ConnectionReadTimeout,
                                        10 => ErrorCode::ConnectionWriteTimeout,
                                        11 => ErrorCode::ConnectionLimitReached,
                                        12 => ErrorCode::TlsProtocolError,
                                        13 => ErrorCode::TlsCertificateError,
                                        14 => {
                                            let e65 = {
                                                let l10 = i32::from(*ptr1.add(16).cast::<u8>());
                                                let l12 = i32::from(*ptr1.add(20).cast::<u8>());
                                                TlsAlertReceivedPayload {
                                                    alert_id: match l10 {
                                                        0 => None,
                                                        1 => {
                                                            let e = {
                                                                let l11 = i32::from(
                                                                    *ptr1.add(17).cast::<u8>(),
                                                                );
                                                                l11 as u8
                                                            };
                                                            Some(e)
                                                        }
                                                        _ => _rt::invalid_enum_discriminant(),
                                                    },
                                                    alert_message: match l12 {
                                                        0 => None,
                                                        1 => {
                                                            let e = {
                                                                let l13 =
                                                                    *ptr1.add(24).cast::<*mut u8>();
                                                                let l14 =
                                                                    *ptr1.add(28).cast::<usize>();
                                                                let len15 = l14;
                                                                let bytes15 =
                                                                    _rt::Vec::from_raw_parts(
                                                                        l13.cast(),
                                                                        len15,
                                                                        len15,
                                                                    );
                                                                _rt::string_lift(bytes15)
                                                            };
                                                            Some(e)
                                                        }
                                                        _ => _rt::invalid_enum_discriminant(),
                                                    },
                                                }
                                            };
                                            ErrorCode::TlsAlertReceived(e65)
                                        }
                                        15 => ErrorCode::HttpRequestDenied,
                                        16 => ErrorCode::HttpRequestLengthRequired,
                                        17 => {
                                            let e65 = {
                                                let l16 = i32::from(*ptr1.add(16).cast::<u8>());
                                                match l16 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l17 = *ptr1.add(24).cast::<i64>();
                                                            l17 as u64
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                }
                                            };
                                            ErrorCode::HttpRequestBodySize(e65)
                                        }
                                        18 => ErrorCode::HttpRequestMethodInvalid,
                                        19 => ErrorCode::HttpRequestUriInvalid,
                                        20 => ErrorCode::HttpRequestUriTooLong,
                                        21 => {
                                            let e65 = {
                                                let l18 = i32::from(*ptr1.add(16).cast::<u8>());
                                                match l18 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l19 = *ptr1.add(20).cast::<i32>();
                                                            l19 as u32
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                }
                                            };
                                            ErrorCode::HttpRequestHeaderSectionSize(e65)
                                        }
                                        22 => {
                                            let e65 = {
                                                let l20 = i32::from(*ptr1.add(16).cast::<u8>());
                                                match l20 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l21 = i32::from(
                                                                *ptr1.add(20).cast::<u8>(),
                                                            );
                                                            let l25 = i32::from(
                                                                *ptr1.add(32).cast::<u8>(),
                                                            );
                                                            FieldSizePayload {
                                                                field_name: match l21 {
                                                                    0 => None,
                                                                    1 => {
                                                                        let e = {
                                                                            let l22 = *ptr1.add(24).cast::<*mut u8>();
                                                                            let l23 = *ptr1.add(28).cast::<usize>();
                                                                            let len24 = l23;
                                                                            let bytes24 = _rt::Vec::from_raw_parts(
                                                                                l22.cast(),
                                                                                len24,
                                                                                len24,
                                                                            );
                                                                            _rt::string_lift(bytes24)
                                                                        };
                                                                        Some(e)
                                                                    }
                                                                    _ => _rt::invalid_enum_discriminant(),
                                                                },
                                                                field_size: match l25 {
                                                                    0 => None,
                                                                    1 => {
                                                                        let e = {
                                                                            let l26 = *ptr1.add(36).cast::<i32>();
                                                                            l26 as u32
                                                                        };
                                                                        Some(e)
                                                                    }
                                                                    _ => _rt::invalid_enum_discriminant(),
                                                                },
                                                            }
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                }
                                            };
                                            ErrorCode::HttpRequestHeaderSize(e65)
                                        }
                                        23 => {
                                            let e65 = {
                                                let l27 = i32::from(*ptr1.add(16).cast::<u8>());
                                                match l27 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l28 = *ptr1.add(20).cast::<i32>();
                                                            l28 as u32
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                }
                                            };
                                            ErrorCode::HttpRequestTrailerSectionSize(e65)
                                        }
                                        24 => {
                                            let e65 = {
                                                let l29 = i32::from(*ptr1.add(16).cast::<u8>());
                                                let l33 = i32::from(*ptr1.add(28).cast::<u8>());
                                                FieldSizePayload {
                                                    field_name: match l29 {
                                                        0 => None,
                                                        1 => {
                                                            let e = {
                                                                let l30 =
                                                                    *ptr1.add(20).cast::<*mut u8>();
                                                                let l31 =
                                                                    *ptr1.add(24).cast::<usize>();
                                                                let len32 = l31;
                                                                let bytes32 =
                                                                    _rt::Vec::from_raw_parts(
                                                                        l30.cast(),
                                                                        len32,
                                                                        len32,
                                                                    );
                                                                _rt::string_lift(bytes32)
                                                            };
                                                            Some(e)
                                                        }
                                                        _ => _rt::invalid_enum_discriminant(),
                                                    },
                                                    field_size: match l33 {
                                                        0 => None,
                                                        1 => {
                                                            let e = {
                                                                let l34 =
                                                                    *ptr1.add(32).cast::<i32>();
                                                                l34 as u32
                                                            };
                                                            Some(e)
                                                        }
                                                        _ => _rt::invalid_enum_discriminant(),
                                                    },
                                                }
                                            };
                                            ErrorCode::HttpRequestTrailerSize(e65)
                                        }
                                        25 => ErrorCode::HttpResponseIncomplete,
                                        26 => {
                                            let e65 = {
                                                let l35 = i32::from(*ptr1.add(16).cast::<u8>());
                                                match l35 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l36 = *ptr1.add(20).cast::<i32>();
                                                            l36 as u32
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                }
                                            };
                                            ErrorCode::HttpResponseHeaderSectionSize(e65)
                                        }
                                        27 => {
                                            let e65 = {
                                                let l37 = i32::from(*ptr1.add(16).cast::<u8>());
                                                let l41 = i32::from(*ptr1.add(28).cast::<u8>());
                                                FieldSizePayload {
                                                    field_name: match l37 {
                                                        0 => None,
                                                        1 => {
                                                            let e = {
                                                                let l38 =
                                                                    *ptr1.add(20).cast::<*mut u8>();
                                                                let l39 =
                                                                    *ptr1.add(24).cast::<usize>();
                                                                let len40 = l39;
                                                                let bytes40 =
                                                                    _rt::Vec::from_raw_parts(
                                                                        l38.cast(),
                                                                        len40,
                                                                        len40,
                                                                    );
                                                                _rt::string_lift(bytes40)
                                                            };
                                                            Some(e)
                                                        }
                                                        _ => _rt::invalid_enum_discriminant(),
                                                    },
                                                    field_size: match l41 {
                                                        0 => None,
                                                        1 => {
                                                            let e = {
                                                                let l42 =
                                                                    *ptr1.add(32).cast::<i32>();
                                                                l42 as u32
                                                            };
                                                            Some(e)
                                                        }
                                                        _ => _rt::invalid_enum_discriminant(),
                                                    },
                                                }
                                            };
                                            ErrorCode::HttpResponseHeaderSize(e65)
                                        }
                                        28 => {
                                            let e65 = {
                                                let l43 = i32::from(*ptr1.add(16).cast::<u8>());
                                                match l43 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l44 = *ptr1.add(24).cast::<i64>();
                                                            l44 as u64
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                }
                                            };
                                            ErrorCode::HttpResponseBodySize(e65)
                                        }
                                        29 => {
                                            let e65 = {
                                                let l45 = i32::from(*ptr1.add(16).cast::<u8>());
                                                match l45 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l46 = *ptr1.add(20).cast::<i32>();
                                                            l46 as u32
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                }
                                            };
                                            ErrorCode::HttpResponseTrailerSectionSize(e65)
                                        }
                                        30 => {
                                            let e65 = {
                                                let l47 = i32::from(*ptr1.add(16).cast::<u8>());
                                                let l51 = i32::from(*ptr1.add(28).cast::<u8>());
                                                FieldSizePayload {
                                                    field_name: match l47 {
                                                        0 => None,
                                                        1 => {
                                                            let e = {
                                                                let l48 =
                                                                    *ptr1.add(20).cast::<*mut u8>();
                                                                let l49 =
                                                                    *ptr1.add(24).cast::<usize>();
                                                                let len50 = l49;
                                                                let bytes50 =
                                                                    _rt::Vec::from_raw_parts(
                                                                        l48.cast(),
                                                                        len50,
                                                                        len50,
                                                                    );
                                                                _rt::string_lift(bytes50)
                                                            };
                                                            Some(e)
                                                        }
                                                        _ => _rt::invalid_enum_discriminant(),
                                                    },
                                                    field_size: match l51 {
                                                        0 => None,
                                                        1 => {
                                                            let e = {
                                                                let l52 =
                                                                    *ptr1.add(32).cast::<i32>();
                                                                l52 as u32
                                                            };
                                                            Some(e)
                                                        }
                                                        _ => _rt::invalid_enum_discriminant(),
                                                    },
                                                }
                                            };
                                            ErrorCode::HttpResponseTrailerSize(e65)
                                        }
                                        31 => {
                                            let e65 = {
                                                let l53 = i32::from(*ptr1.add(16).cast::<u8>());
                                                match l53 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l54 =
                                                                *ptr1.add(20).cast::<*mut u8>();
                                                            let l55 = *ptr1.add(24).cast::<usize>();
                                                            let len56 = l55;
                                                            let bytes56 = _rt::Vec::from_raw_parts(
                                                                l54.cast(),
                                                                len56,
                                                                len56,
                                                            );
                                                            _rt::string_lift(bytes56)
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                }
                                            };
                                            ErrorCode::HttpResponseTransferCoding(e65)
                                        }
                                        32 => {
                                            let e65 = {
                                                let l57 = i32::from(*ptr1.add(16).cast::<u8>());
                                                match l57 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l58 =
                                                                *ptr1.add(20).cast::<*mut u8>();
                                                            let l59 = *ptr1.add(24).cast::<usize>();
                                                            let len60 = l59;
                                                            let bytes60 = _rt::Vec::from_raw_parts(
                                                                l58.cast(),
                                                                len60,
                                                                len60,
                                                            );
                                                            _rt::string_lift(bytes60)
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                }
                                            };
                                            ErrorCode::HttpResponseContentCoding(e65)
                                        }
                                        33 => ErrorCode::HttpResponseTimeout,
                                        34 => ErrorCode::HttpUpgradeFailed,
                                        35 => ErrorCode::HttpProtocolError,
                                        36 => ErrorCode::LoopDetected,
                                        37 => ErrorCode::ConfigurationError,
                                        n => {
                                            debug_assert_eq!(n, 38, "invalid enum discriminant");
                                            let e65 = {
                                                let l61 = i32::from(*ptr1.add(16).cast::<u8>());
                                                match l61 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l62 =
                                                                *ptr1.add(20).cast::<*mut u8>();
                                                            let l63 = *ptr1.add(24).cast::<usize>();
                                                            let len64 = l63;
                                                            let bytes64 = _rt::Vec::from_raw_parts(
                                                                l62.cast(),
                                                                len64,
                                                                len64,
                                                            );
                                                            _rt::string_lift(bytes64)
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                }
                                            };
                                            ErrorCode::InternalError(e65)
                                        }
                                    };
                                    v65
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl FutureIncomingResponse {
                #[allow(unused_unsafe, clippy::all)]
                pub fn subscribe(&self) -> Pollable {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]future-incoming-response.subscribe"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32);
                        super::super::super::wasi::io::poll::Pollable::from_handle(ret as u32)
                    }
                }
            }
            impl FutureIncomingResponse {
                #[allow(unused_unsafe, clippy::all)]
                pub fn get(&self) -> Option<Result<Result<IncomingResponse, ErrorCode>, ()>> {
                    unsafe {
                        #[repr(align(8))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 56]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 56]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]future-incoming-response.get"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => None,
                            1 => {
                                let e = {
                                    let l2 = i32::from(*ptr0.add(8).cast::<u8>());
                                    match l2 {
                                        0 => {
                                            let e = {
                                                let l3 = i32::from(*ptr0.add(16).cast::<u8>());
                                                match l3 {
                                                    0 => {
                                                        let e = {
                                                            let l4 = *ptr0.add(24).cast::<i32>();
                                                            IncomingResponse::from_handle(l4 as u32)
                                                        };
                                                        Ok(e)
                                                    }
                                                    1 => {
                                                        let e = {
                                                            let l5 = i32::from(
                                                                *ptr0.add(24).cast::<u8>(),
                                                            );
                                                            let v67 = match l5 {
                                                                0 => ErrorCode::DnsTimeout,
                                                                1 => {
                                                                    let e67 = {
                                                                        let l6 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        let l10 = i32::from(*ptr0.add(44).cast::<u8>());
                                                                        DnsErrorPayload {
                                                                            rcode: match l6 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l7 = *ptr0.add(36).cast::<*mut u8>();
                                                                                        let l8 = *ptr0.add(40).cast::<usize>();
                                                                                        let len9 = l8;
                                                                                        let bytes9 = _rt::Vec::from_raw_parts(
                                                                                            l7.cast(),
                                                                                            len9,
                                                                                            len9,
                                                                                        );
                                                                                        _rt::string_lift(bytes9)
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                            info_code: match l10 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l11 = i32::from(*ptr0.add(46).cast::<u16>());
                                                                                        l11 as u16
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                        }
                                                                    };
                                                                    ErrorCode::DnsError(e67)
                                                                }
                                                                2 => ErrorCode::DestinationNotFound,
                                                                3 => ErrorCode::DestinationUnavailable,
                                                                4 => ErrorCode::DestinationIpProhibited,
                                                                5 => ErrorCode::DestinationIpUnroutable,
                                                                6 => ErrorCode::ConnectionRefused,
                                                                7 => ErrorCode::ConnectionTerminated,
                                                                8 => ErrorCode::ConnectionTimeout,
                                                                9 => ErrorCode::ConnectionReadTimeout,
                                                                10 => ErrorCode::ConnectionWriteTimeout,
                                                                11 => ErrorCode::ConnectionLimitReached,
                                                                12 => ErrorCode::TlsProtocolError,
                                                                13 => ErrorCode::TlsCertificateError,
                                                                14 => {
                                                                    let e67 = {
                                                                        let l12 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        let l14 = i32::from(*ptr0.add(36).cast::<u8>());
                                                                        TlsAlertReceivedPayload {
                                                                            alert_id: match l12 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l13 = i32::from(*ptr0.add(33).cast::<u8>());
                                                                                        l13 as u8
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                            alert_message: match l14 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l15 = *ptr0.add(40).cast::<*mut u8>();
                                                                                        let l16 = *ptr0.add(44).cast::<usize>();
                                                                                        let len17 = l16;
                                                                                        let bytes17 = _rt::Vec::from_raw_parts(
                                                                                            l15.cast(),
                                                                                            len17,
                                                                                            len17,
                                                                                        );
                                                                                        _rt::string_lift(bytes17)
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                        }
                                                                    };
                                                                    ErrorCode::TlsAlertReceived(e67)
                                                                }
                                                                15 => ErrorCode::HttpRequestDenied,
                                                                16 => ErrorCode::HttpRequestLengthRequired,
                                                                17 => {
                                                                    let e67 = {
                                                                        let l18 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l18 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l19 = *ptr0.add(40).cast::<i64>();
                                                                                    l19 as u64
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpRequestBodySize(e67)
                                                                }
                                                                18 => ErrorCode::HttpRequestMethodInvalid,
                                                                19 => ErrorCode::HttpRequestUriInvalid,
                                                                20 => ErrorCode::HttpRequestUriTooLong,
                                                                21 => {
                                                                    let e67 = {
                                                                        let l20 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l20 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l21 = *ptr0.add(36).cast::<i32>();
                                                                                    l21 as u32
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpRequestHeaderSectionSize(e67)
                                                                }
                                                                22 => {
                                                                    let e67 = {
                                                                        let l22 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l22 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l23 = i32::from(*ptr0.add(36).cast::<u8>());
                                                                                    let l27 = i32::from(*ptr0.add(48).cast::<u8>());
                                                                                    FieldSizePayload {
                                                                                        field_name: match l23 {
                                                                                            0 => None,
                                                                                            1 => {
                                                                                                let e = {
                                                                                                    let l24 = *ptr0.add(40).cast::<*mut u8>();
                                                                                                    let l25 = *ptr0.add(44).cast::<usize>();
                                                                                                    let len26 = l25;
                                                                                                    let bytes26 = _rt::Vec::from_raw_parts(
                                                                                                        l24.cast(),
                                                                                                        len26,
                                                                                                        len26,
                                                                                                    );
                                                                                                    _rt::string_lift(bytes26)
                                                                                                };
                                                                                                Some(e)
                                                                                            }
                                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                                        },
                                                                                        field_size: match l27 {
                                                                                            0 => None,
                                                                                            1 => {
                                                                                                let e = {
                                                                                                    let l28 = *ptr0.add(52).cast::<i32>();
                                                                                                    l28 as u32
                                                                                                };
                                                                                                Some(e)
                                                                                            }
                                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                                        },
                                                                                    }
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpRequestHeaderSize(e67)
                                                                }
                                                                23 => {
                                                                    let e67 = {
                                                                        let l29 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l29 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l30 = *ptr0.add(36).cast::<i32>();
                                                                                    l30 as u32
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpRequestTrailerSectionSize(e67)
                                                                }
                                                                24 => {
                                                                    let e67 = {
                                                                        let l31 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        let l35 = i32::from(*ptr0.add(44).cast::<u8>());
                                                                        FieldSizePayload {
                                                                            field_name: match l31 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l32 = *ptr0.add(36).cast::<*mut u8>();
                                                                                        let l33 = *ptr0.add(40).cast::<usize>();
                                                                                        let len34 = l33;
                                                                                        let bytes34 = _rt::Vec::from_raw_parts(
                                                                                            l32.cast(),
                                                                                            len34,
                                                                                            len34,
                                                                                        );
                                                                                        _rt::string_lift(bytes34)
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                            field_size: match l35 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l36 = *ptr0.add(48).cast::<i32>();
                                                                                        l36 as u32
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpRequestTrailerSize(e67)
                                                                }
                                                                25 => ErrorCode::HttpResponseIncomplete,
                                                                26 => {
                                                                    let e67 = {
                                                                        let l37 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l37 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l38 = *ptr0.add(36).cast::<i32>();
                                                                                    l38 as u32
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseHeaderSectionSize(e67)
                                                                }
                                                                27 => {
                                                                    let e67 = {
                                                                        let l39 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        let l43 = i32::from(*ptr0.add(44).cast::<u8>());
                                                                        FieldSizePayload {
                                                                            field_name: match l39 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l40 = *ptr0.add(36).cast::<*mut u8>();
                                                                                        let l41 = *ptr0.add(40).cast::<usize>();
                                                                                        let len42 = l41;
                                                                                        let bytes42 = _rt::Vec::from_raw_parts(
                                                                                            l40.cast(),
                                                                                            len42,
                                                                                            len42,
                                                                                        );
                                                                                        _rt::string_lift(bytes42)
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                            field_size: match l43 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l44 = *ptr0.add(48).cast::<i32>();
                                                                                        l44 as u32
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseHeaderSize(e67)
                                                                }
                                                                28 => {
                                                                    let e67 = {
                                                                        let l45 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l45 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l46 = *ptr0.add(40).cast::<i64>();
                                                                                    l46 as u64
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseBodySize(e67)
                                                                }
                                                                29 => {
                                                                    let e67 = {
                                                                        let l47 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l47 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l48 = *ptr0.add(36).cast::<i32>();
                                                                                    l48 as u32
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseTrailerSectionSize(e67)
                                                                }
                                                                30 => {
                                                                    let e67 = {
                                                                        let l49 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        let l53 = i32::from(*ptr0.add(44).cast::<u8>());
                                                                        FieldSizePayload {
                                                                            field_name: match l49 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l50 = *ptr0.add(36).cast::<*mut u8>();
                                                                                        let l51 = *ptr0.add(40).cast::<usize>();
                                                                                        let len52 = l51;
                                                                                        let bytes52 = _rt::Vec::from_raw_parts(
                                                                                            l50.cast(),
                                                                                            len52,
                                                                                            len52,
                                                                                        );
                                                                                        _rt::string_lift(bytes52)
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                            field_size: match l53 {
                                                                                0 => None,
                                                                                1 => {
                                                                                    let e = {
                                                                                        let l54 = *ptr0.add(48).cast::<i32>();
                                                                                        l54 as u32
                                                                                    };
                                                                                    Some(e)
                                                                                }
                                                                                _ => _rt::invalid_enum_discriminant(),
                                                                            },
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseTrailerSize(e67)
                                                                }
                                                                31 => {
                                                                    let e67 = {
                                                                        let l55 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l55 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l56 = *ptr0.add(36).cast::<*mut u8>();
                                                                                    let l57 = *ptr0.add(40).cast::<usize>();
                                                                                    let len58 = l57;
                                                                                    let bytes58 = _rt::Vec::from_raw_parts(
                                                                                        l56.cast(),
                                                                                        len58,
                                                                                        len58,
                                                                                    );
                                                                                    _rt::string_lift(bytes58)
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseTransferCoding(e67)
                                                                }
                                                                32 => {
                                                                    let e67 = {
                                                                        let l59 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l59 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l60 = *ptr0.add(36).cast::<*mut u8>();
                                                                                    let l61 = *ptr0.add(40).cast::<usize>();
                                                                                    let len62 = l61;
                                                                                    let bytes62 = _rt::Vec::from_raw_parts(
                                                                                        l60.cast(),
                                                                                        len62,
                                                                                        len62,
                                                                                    );
                                                                                    _rt::string_lift(bytes62)
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::HttpResponseContentCoding(e67)
                                                                }
                                                                33 => ErrorCode::HttpResponseTimeout,
                                                                34 => ErrorCode::HttpUpgradeFailed,
                                                                35 => ErrorCode::HttpProtocolError,
                                                                36 => ErrorCode::LoopDetected,
                                                                37 => ErrorCode::ConfigurationError,
                                                                n => {
                                                                    debug_assert_eq!(n, 38, "invalid enum discriminant");
                                                                    let e67 = {
                                                                        let l63 = i32::from(*ptr0.add(32).cast::<u8>());
                                                                        match l63 {
                                                                            0 => None,
                                                                            1 => {
                                                                                let e = {
                                                                                    let l64 = *ptr0.add(36).cast::<*mut u8>();
                                                                                    let l65 = *ptr0.add(40).cast::<usize>();
                                                                                    let len66 = l65;
                                                                                    let bytes66 = _rt::Vec::from_raw_parts(
                                                                                        l64.cast(),
                                                                                        len66,
                                                                                        len66,
                                                                                    );
                                                                                    _rt::string_lift(bytes66)
                                                                                };
                                                                                Some(e)
                                                                            }
                                                                            _ => _rt::invalid_enum_discriminant(),
                                                                        }
                                                                    };
                                                                    ErrorCode::InternalError(e67)
                                                                }
                                                            };
                                                            v67
                                                        };
                                                        Err(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                }
                                            };
                                            Ok(e)
                                        }
                                        1 => {
                                            let e = ();
                                            Err(e)
                                        }
                                        _ => _rt::invalid_enum_discriminant(),
                                    }
                                };
                                Some(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            #[allow(unused_unsafe, clippy::all)]
            pub fn http_error_code(err: &IoError) -> Option<ErrorCode> {
                unsafe {
                    #[repr(align(8))]
                    struct RetArea([::core::mem::MaybeUninit<u8>; 40]);
                    let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 40]);
                    let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                    #[cfg(target_arch = "wasm32")]
                    #[link(wasm_import_module = "wasi:http/types@0.2.0")]
                    extern "C" {
                        #[link_name = "http-error-code"]
                        fn wit_import(_: i32, _: *mut u8);
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    fn wit_import(_: i32, _: *mut u8) {
                        unreachable!()
                    }
                    wit_import((err).handle() as i32, ptr0);
                    let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                    match l1 {
                        0 => None,
                        1 => {
                            let e = {
                                let l2 = i32::from(*ptr0.add(8).cast::<u8>());
                                let v64 = match l2 {
                                    0 => ErrorCode::DnsTimeout,
                                    1 => {
                                        let e64 = {
                                            let l3 = i32::from(*ptr0.add(16).cast::<u8>());
                                            let l7 = i32::from(*ptr0.add(28).cast::<u8>());
                                            DnsErrorPayload {
                                                rcode: match l3 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l4 =
                                                                *ptr0.add(20).cast::<*mut u8>();
                                                            let l5 = *ptr0.add(24).cast::<usize>();
                                                            let len6 = l5;
                                                            let bytes6 = _rt::Vec::from_raw_parts(
                                                                l4.cast(),
                                                                len6,
                                                                len6,
                                                            );
                                                            _rt::string_lift(bytes6)
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                                info_code: match l7 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l8 = i32::from(
                                                                *ptr0.add(30).cast::<u16>(),
                                                            );
                                                            l8 as u16
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                            }
                                        };
                                        ErrorCode::DnsError(e64)
                                    }
                                    2 => ErrorCode::DestinationNotFound,
                                    3 => ErrorCode::DestinationUnavailable,
                                    4 => ErrorCode::DestinationIpProhibited,
                                    5 => ErrorCode::DestinationIpUnroutable,
                                    6 => ErrorCode::ConnectionRefused,
                                    7 => ErrorCode::ConnectionTerminated,
                                    8 => ErrorCode::ConnectionTimeout,
                                    9 => ErrorCode::ConnectionReadTimeout,
                                    10 => ErrorCode::ConnectionWriteTimeout,
                                    11 => ErrorCode::ConnectionLimitReached,
                                    12 => ErrorCode::TlsProtocolError,
                                    13 => ErrorCode::TlsCertificateError,
                                    14 => {
                                        let e64 = {
                                            let l9 = i32::from(*ptr0.add(16).cast::<u8>());
                                            let l11 = i32::from(*ptr0.add(20).cast::<u8>());
                                            TlsAlertReceivedPayload {
                                                alert_id: match l9 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l10 = i32::from(
                                                                *ptr0.add(17).cast::<u8>(),
                                                            );
                                                            l10 as u8
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                                alert_message: match l11 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l12 =
                                                                *ptr0.add(24).cast::<*mut u8>();
                                                            let l13 = *ptr0.add(28).cast::<usize>();
                                                            let len14 = l13;
                                                            let bytes14 = _rt::Vec::from_raw_parts(
                                                                l12.cast(),
                                                                len14,
                                                                len14,
                                                            );
                                                            _rt::string_lift(bytes14)
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                            }
                                        };
                                        ErrorCode::TlsAlertReceived(e64)
                                    }
                                    15 => ErrorCode::HttpRequestDenied,
                                    16 => ErrorCode::HttpRequestLengthRequired,
                                    17 => {
                                        let e64 = {
                                            let l15 = i32::from(*ptr0.add(16).cast::<u8>());
                                            match l15 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l16 = *ptr0.add(24).cast::<i64>();
                                                        l16 as u64
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        ErrorCode::HttpRequestBodySize(e64)
                                    }
                                    18 => ErrorCode::HttpRequestMethodInvalid,
                                    19 => ErrorCode::HttpRequestUriInvalid,
                                    20 => ErrorCode::HttpRequestUriTooLong,
                                    21 => {
                                        let e64 = {
                                            let l17 = i32::from(*ptr0.add(16).cast::<u8>());
                                            match l17 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l18 = *ptr0.add(20).cast::<i32>();
                                                        l18 as u32
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        ErrorCode::HttpRequestHeaderSectionSize(e64)
                                    }
                                    22 => {
                                        let e64 = {
                                            let l19 = i32::from(*ptr0.add(16).cast::<u8>());
                                            match l19 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l20 =
                                                            i32::from(*ptr0.add(20).cast::<u8>());
                                                        let l24 =
                                                            i32::from(*ptr0.add(32).cast::<u8>());
                                                        FieldSizePayload {
                                                            field_name: match l20 {
                                                                0 => None,
                                                                1 => {
                                                                    let e = {
                                                                        let l21 = *ptr0
                                                                            .add(24)
                                                                            .cast::<*mut u8>(
                                                                        );
                                                                        let l22 = *ptr0
                                                                            .add(28)
                                                                            .cast::<usize>();
                                                                        let len23 = l22;
                                                                        let bytes23 = _rt::Vec::from_raw_parts(
                                                                            l21.cast(),
                                                                            len23,
                                                                            len23,
                                                                        );
                                                                        _rt::string_lift(bytes23)
                                                                    };
                                                                    Some(e)
                                                                }
                                                                _ => {
                                                                    _rt::invalid_enum_discriminant()
                                                                }
                                                            },
                                                            field_size: match l24 {
                                                                0 => None,
                                                                1 => {
                                                                    let e = {
                                                                        let l25 = *ptr0
                                                                            .add(36)
                                                                            .cast::<i32>();
                                                                        l25 as u32
                                                                    };
                                                                    Some(e)
                                                                }
                                                                _ => {
                                                                    _rt::invalid_enum_discriminant()
                                                                }
                                                            },
                                                        }
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        ErrorCode::HttpRequestHeaderSize(e64)
                                    }
                                    23 => {
                                        let e64 = {
                                            let l26 = i32::from(*ptr0.add(16).cast::<u8>());
                                            match l26 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l27 = *ptr0.add(20).cast::<i32>();
                                                        l27 as u32
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        ErrorCode::HttpRequestTrailerSectionSize(e64)
                                    }
                                    24 => {
                                        let e64 = {
                                            let l28 = i32::from(*ptr0.add(16).cast::<u8>());
                                            let l32 = i32::from(*ptr0.add(28).cast::<u8>());
                                            FieldSizePayload {
                                                field_name: match l28 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l29 =
                                                                *ptr0.add(20).cast::<*mut u8>();
                                                            let l30 = *ptr0.add(24).cast::<usize>();
                                                            let len31 = l30;
                                                            let bytes31 = _rt::Vec::from_raw_parts(
                                                                l29.cast(),
                                                                len31,
                                                                len31,
                                                            );
                                                            _rt::string_lift(bytes31)
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                                field_size: match l32 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l33 = *ptr0.add(32).cast::<i32>();
                                                            l33 as u32
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                            }
                                        };
                                        ErrorCode::HttpRequestTrailerSize(e64)
                                    }
                                    25 => ErrorCode::HttpResponseIncomplete,
                                    26 => {
                                        let e64 = {
                                            let l34 = i32::from(*ptr0.add(16).cast::<u8>());
                                            match l34 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l35 = *ptr0.add(20).cast::<i32>();
                                                        l35 as u32
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        ErrorCode::HttpResponseHeaderSectionSize(e64)
                                    }
                                    27 => {
                                        let e64 = {
                                            let l36 = i32::from(*ptr0.add(16).cast::<u8>());
                                            let l40 = i32::from(*ptr0.add(28).cast::<u8>());
                                            FieldSizePayload {
                                                field_name: match l36 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l37 =
                                                                *ptr0.add(20).cast::<*mut u8>();
                                                            let l38 = *ptr0.add(24).cast::<usize>();
                                                            let len39 = l38;
                                                            let bytes39 = _rt::Vec::from_raw_parts(
                                                                l37.cast(),
                                                                len39,
                                                                len39,
                                                            );
                                                            _rt::string_lift(bytes39)
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                                field_size: match l40 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l41 = *ptr0.add(32).cast::<i32>();
                                                            l41 as u32
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                            }
                                        };
                                        ErrorCode::HttpResponseHeaderSize(e64)
                                    }
                                    28 => {
                                        let e64 = {
                                            let l42 = i32::from(*ptr0.add(16).cast::<u8>());
                                            match l42 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l43 = *ptr0.add(24).cast::<i64>();
                                                        l43 as u64
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        ErrorCode::HttpResponseBodySize(e64)
                                    }
                                    29 => {
                                        let e64 = {
                                            let l44 = i32::from(*ptr0.add(16).cast::<u8>());
                                            match l44 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l45 = *ptr0.add(20).cast::<i32>();
                                                        l45 as u32
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        ErrorCode::HttpResponseTrailerSectionSize(e64)
                                    }
                                    30 => {
                                        let e64 = {
                                            let l46 = i32::from(*ptr0.add(16).cast::<u8>());
                                            let l50 = i32::from(*ptr0.add(28).cast::<u8>());
                                            FieldSizePayload {
                                                field_name: match l46 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l47 =
                                                                *ptr0.add(20).cast::<*mut u8>();
                                                            let l48 = *ptr0.add(24).cast::<usize>();
                                                            let len49 = l48;
                                                            let bytes49 = _rt::Vec::from_raw_parts(
                                                                l47.cast(),
                                                                len49,
                                                                len49,
                                                            );
                                                            _rt::string_lift(bytes49)
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                                field_size: match l50 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l51 = *ptr0.add(32).cast::<i32>();
                                                            l51 as u32
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                            }
                                        };
                                        ErrorCode::HttpResponseTrailerSize(e64)
                                    }
                                    31 => {
                                        let e64 = {
                                            let l52 = i32::from(*ptr0.add(16).cast::<u8>());
                                            match l52 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l53 = *ptr0.add(20).cast::<*mut u8>();
                                                        let l54 = *ptr0.add(24).cast::<usize>();
                                                        let len55 = l54;
                                                        let bytes55 = _rt::Vec::from_raw_parts(
                                                            l53.cast(),
                                                            len55,
                                                            len55,
                                                        );
                                                        _rt::string_lift(bytes55)
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        ErrorCode::HttpResponseTransferCoding(e64)
                                    }
                                    32 => {
                                        let e64 = {
                                            let l56 = i32::from(*ptr0.add(16).cast::<u8>());
                                            match l56 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l57 = *ptr0.add(20).cast::<*mut u8>();
                                                        let l58 = *ptr0.add(24).cast::<usize>();
                                                        let len59 = l58;
                                                        let bytes59 = _rt::Vec::from_raw_parts(
                                                            l57.cast(),
                                                            len59,
                                                            len59,
                                                        );
                                                        _rt::string_lift(bytes59)
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        ErrorCode::HttpResponseContentCoding(e64)
                                    }
                                    33 => ErrorCode::HttpResponseTimeout,
                                    34 => ErrorCode::HttpUpgradeFailed,
                                    35 => ErrorCode::HttpProtocolError,
                                    36 => ErrorCode::LoopDetected,
                                    37 => ErrorCode::ConfigurationError,
                                    n => {
                                        debug_assert_eq!(n, 38, "invalid enum discriminant");
                                        let e64 = {
                                            let l60 = i32::from(*ptr0.add(16).cast::<u8>());
                                            match l60 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l61 = *ptr0.add(20).cast::<*mut u8>();
                                                        let l62 = *ptr0.add(24).cast::<usize>();
                                                        let len63 = l62;
                                                        let bytes63 = _rt::Vec::from_raw_parts(
                                                            l61.cast(),
                                                            len63,
                                                            len63,
                                                        );
                                                        _rt::string_lift(bytes63)
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        ErrorCode::InternalError(e64)
                                    }
                                };
                                v64
                            };
                            Some(e)
                        }
                        _ => _rt::invalid_enum_discriminant(),
                    }
                }
            }
        }
        #[allow(dead_code, clippy::all)]
        pub mod outgoing_handler {
            #[used]
            #[doc(hidden)]
            static __FORCE_SECTION_REF: fn() =
                super::super::super::__link_custom_section_describing_imports;
            use super::super::super::_rt;
            pub type OutgoingRequest = super::super::super::wasi::http::types::OutgoingRequest;
            pub type RequestOptions = super::super::super::wasi::http::types::RequestOptions;
            pub type FutureIncomingResponse =
                super::super::super::wasi::http::types::FutureIncomingResponse;
            pub type ErrorCode = super::super::super::wasi::http::types::ErrorCode;
            #[allow(unused_unsafe, clippy::all)]
            pub fn handle(
                request: OutgoingRequest,
                options: Option<RequestOptions>,
            ) -> Result<FutureIncomingResponse, ErrorCode> {
                unsafe {
                    #[repr(align(8))]
                    struct RetArea([::core::mem::MaybeUninit<u8>; 40]);
                    let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 40]);
                    let (result0_0, result0_1) = match &options {
                        Some(e) => (1i32, (e).take_handle() as i32),
                        None => (0i32, 0i32),
                    };
                    let ptr1 = ret_area.0.as_mut_ptr().cast::<u8>();
                    #[cfg(target_arch = "wasm32")]
                    #[link(wasm_import_module = "wasi:http/outgoing-handler@0.2.0")]
                    extern "C" {
                        #[link_name = "handle"]
                        fn wit_import(_: i32, _: i32, _: i32, _: *mut u8);
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    fn wit_import(_: i32, _: i32, _: i32, _: *mut u8) {
                        unreachable!()
                    }
                    wit_import((&request).take_handle() as i32, result0_0, result0_1, ptr1);
                    let l2 = i32::from(*ptr1.add(0).cast::<u8>());
                    match l2 {
                        0 => {
                            let e = {
                                let l3 = *ptr1.add(8).cast::<i32>();
                                super::super::super::wasi::http::types::FutureIncomingResponse::from_handle(
                                    l3 as u32,
                                )
                            };
                            Ok(e)
                        }
                        1 => {
                            let e = {
                                let l4 = i32::from(*ptr1.add(8).cast::<u8>());
                                use super::super::super::wasi::http::types::ErrorCode as V66;
                                let v66 = match l4 {
                                    0 => V66::DnsTimeout,
                                    1 => {
                                        let e66 = {
                                            let l5 = i32::from(*ptr1.add(16).cast::<u8>());
                                            let l9 = i32::from(*ptr1.add(28).cast::<u8>());
                                            super::super::super::wasi::http::types::DnsErrorPayload {
                                                rcode: match l5 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l6 = *ptr1.add(20).cast::<*mut u8>();
                                                            let l7 = *ptr1.add(24).cast::<usize>();
                                                            let len8 = l7;
                                                            let bytes8 = _rt::Vec::from_raw_parts(
                                                                l6.cast(),
                                                                len8,
                                                                len8,
                                                            );
                                                            _rt::string_lift(bytes8)
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                                info_code: match l9 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l10 = i32::from(*ptr1.add(30).cast::<u16>());
                                                            l10 as u16
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                            }
                                        };
                                        V66::DnsError(e66)
                                    }
                                    2 => V66::DestinationNotFound,
                                    3 => V66::DestinationUnavailable,
                                    4 => V66::DestinationIpProhibited,
                                    5 => V66::DestinationIpUnroutable,
                                    6 => V66::ConnectionRefused,
                                    7 => V66::ConnectionTerminated,
                                    8 => V66::ConnectionTimeout,
                                    9 => V66::ConnectionReadTimeout,
                                    10 => V66::ConnectionWriteTimeout,
                                    11 => V66::ConnectionLimitReached,
                                    12 => V66::TlsProtocolError,
                                    13 => V66::TlsCertificateError,
                                    14 => {
                                        let e66 = {
                                            let l11 = i32::from(*ptr1.add(16).cast::<u8>());
                                            let l13 = i32::from(*ptr1.add(20).cast::<u8>());
                                            super::super::super::wasi::http::types::TlsAlertReceivedPayload {
                                                alert_id: match l11 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l12 = i32::from(*ptr1.add(17).cast::<u8>());
                                                            l12 as u8
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                                alert_message: match l13 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l14 = *ptr1.add(24).cast::<*mut u8>();
                                                            let l15 = *ptr1.add(28).cast::<usize>();
                                                            let len16 = l15;
                                                            let bytes16 = _rt::Vec::from_raw_parts(
                                                                l14.cast(),
                                                                len16,
                                                                len16,
                                                            );
                                                            _rt::string_lift(bytes16)
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                            }
                                        };
                                        V66::TlsAlertReceived(e66)
                                    }
                                    15 => V66::HttpRequestDenied,
                                    16 => V66::HttpRequestLengthRequired,
                                    17 => {
                                        let e66 = {
                                            let l17 = i32::from(*ptr1.add(16).cast::<u8>());
                                            match l17 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l18 = *ptr1.add(24).cast::<i64>();
                                                        l18 as u64
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        V66::HttpRequestBodySize(e66)
                                    }
                                    18 => V66::HttpRequestMethodInvalid,
                                    19 => V66::HttpRequestUriInvalid,
                                    20 => V66::HttpRequestUriTooLong,
                                    21 => {
                                        let e66 = {
                                            let l19 = i32::from(*ptr1.add(16).cast::<u8>());
                                            match l19 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l20 = *ptr1.add(20).cast::<i32>();
                                                        l20 as u32
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        V66::HttpRequestHeaderSectionSize(e66)
                                    }
                                    22 => {
                                        let e66 = {
                                            let l21 = i32::from(*ptr1.add(16).cast::<u8>());
                                            match l21 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l22 =
                                                            i32::from(*ptr1.add(20).cast::<u8>());
                                                        let l26 =
                                                            i32::from(*ptr1.add(32).cast::<u8>());
                                                        super::super::super::wasi::http::types::FieldSizePayload {
                                                            field_name: match l22 {
                                                                0 => None,
                                                                1 => {
                                                                    let e = {
                                                                        let l23 = *ptr1.add(24).cast::<*mut u8>();
                                                                        let l24 = *ptr1.add(28).cast::<usize>();
                                                                        let len25 = l24;
                                                                        let bytes25 = _rt::Vec::from_raw_parts(
                                                                            l23.cast(),
                                                                            len25,
                                                                            len25,
                                                                        );
                                                                        _rt::string_lift(bytes25)
                                                                    };
                                                                    Some(e)
                                                                }
                                                                _ => _rt::invalid_enum_discriminant(),
                                                            },
                                                            field_size: match l26 {
                                                                0 => None,
                                                                1 => {
                                                                    let e = {
                                                                        let l27 = *ptr1.add(36).cast::<i32>();
                                                                        l27 as u32
                                                                    };
                                                                    Some(e)
                                                                }
                                                                _ => _rt::invalid_enum_discriminant(),
                                                            },
                                                        }
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        V66::HttpRequestHeaderSize(e66)
                                    }
                                    23 => {
                                        let e66 = {
                                            let l28 = i32::from(*ptr1.add(16).cast::<u8>());
                                            match l28 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l29 = *ptr1.add(20).cast::<i32>();
                                                        l29 as u32
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        V66::HttpRequestTrailerSectionSize(e66)
                                    }
                                    24 => {
                                        let e66 = {
                                            let l30 = i32::from(*ptr1.add(16).cast::<u8>());
                                            let l34 = i32::from(*ptr1.add(28).cast::<u8>());
                                            super::super::super::wasi::http::types::FieldSizePayload {
                                                field_name: match l30 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l31 = *ptr1.add(20).cast::<*mut u8>();
                                                            let l32 = *ptr1.add(24).cast::<usize>();
                                                            let len33 = l32;
                                                            let bytes33 = _rt::Vec::from_raw_parts(
                                                                l31.cast(),
                                                                len33,
                                                                len33,
                                                            );
                                                            _rt::string_lift(bytes33)
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                                field_size: match l34 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l35 = *ptr1.add(32).cast::<i32>();
                                                            l35 as u32
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                            }
                                        };
                                        V66::HttpRequestTrailerSize(e66)
                                    }
                                    25 => V66::HttpResponseIncomplete,
                                    26 => {
                                        let e66 = {
                                            let l36 = i32::from(*ptr1.add(16).cast::<u8>());
                                            match l36 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l37 = *ptr1.add(20).cast::<i32>();
                                                        l37 as u32
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        V66::HttpResponseHeaderSectionSize(e66)
                                    }
                                    27 => {
                                        let e66 = {
                                            let l38 = i32::from(*ptr1.add(16).cast::<u8>());
                                            let l42 = i32::from(*ptr1.add(28).cast::<u8>());
                                            super::super::super::wasi::http::types::FieldSizePayload {
                                                field_name: match l38 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l39 = *ptr1.add(20).cast::<*mut u8>();
                                                            let l40 = *ptr1.add(24).cast::<usize>();
                                                            let len41 = l40;
                                                            let bytes41 = _rt::Vec::from_raw_parts(
                                                                l39.cast(),
                                                                len41,
                                                                len41,
                                                            );
                                                            _rt::string_lift(bytes41)
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                                field_size: match l42 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l43 = *ptr1.add(32).cast::<i32>();
                                                            l43 as u32
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                            }
                                        };
                                        V66::HttpResponseHeaderSize(e66)
                                    }
                                    28 => {
                                        let e66 = {
                                            let l44 = i32::from(*ptr1.add(16).cast::<u8>());
                                            match l44 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l45 = *ptr1.add(24).cast::<i64>();
                                                        l45 as u64
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        V66::HttpResponseBodySize(e66)
                                    }
                                    29 => {
                                        let e66 = {
                                            let l46 = i32::from(*ptr1.add(16).cast::<u8>());
                                            match l46 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l47 = *ptr1.add(20).cast::<i32>();
                                                        l47 as u32
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        V66::HttpResponseTrailerSectionSize(e66)
                                    }
                                    30 => {
                                        let e66 = {
                                            let l48 = i32::from(*ptr1.add(16).cast::<u8>());
                                            let l52 = i32::from(*ptr1.add(28).cast::<u8>());
                                            super::super::super::wasi::http::types::FieldSizePayload {
                                                field_name: match l48 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l49 = *ptr1.add(20).cast::<*mut u8>();
                                                            let l50 = *ptr1.add(24).cast::<usize>();
                                                            let len51 = l50;
                                                            let bytes51 = _rt::Vec::from_raw_parts(
                                                                l49.cast(),
                                                                len51,
                                                                len51,
                                                            );
                                                            _rt::string_lift(bytes51)
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                                field_size: match l52 {
                                                    0 => None,
                                                    1 => {
                                                        let e = {
                                                            let l53 = *ptr1.add(32).cast::<i32>();
                                                            l53 as u32
                                                        };
                                                        Some(e)
                                                    }
                                                    _ => _rt::invalid_enum_discriminant(),
                                                },
                                            }
                                        };
                                        V66::HttpResponseTrailerSize(e66)
                                    }
                                    31 => {
                                        let e66 = {
                                            let l54 = i32::from(*ptr1.add(16).cast::<u8>());
                                            match l54 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l55 = *ptr1.add(20).cast::<*mut u8>();
                                                        let l56 = *ptr1.add(24).cast::<usize>();
                                                        let len57 = l56;
                                                        let bytes57 = _rt::Vec::from_raw_parts(
                                                            l55.cast(),
                                                            len57,
                                                            len57,
                                                        );
                                                        _rt::string_lift(bytes57)
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        V66::HttpResponseTransferCoding(e66)
                                    }
                                    32 => {
                                        let e66 = {
                                            let l58 = i32::from(*ptr1.add(16).cast::<u8>());
                                            match l58 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l59 = *ptr1.add(20).cast::<*mut u8>();
                                                        let l60 = *ptr1.add(24).cast::<usize>();
                                                        let len61 = l60;
                                                        let bytes61 = _rt::Vec::from_raw_parts(
                                                            l59.cast(),
                                                            len61,
                                                            len61,
                                                        );
                                                        _rt::string_lift(bytes61)
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        V66::HttpResponseContentCoding(e66)
                                    }
                                    33 => V66::HttpResponseTimeout,
                                    34 => V66::HttpUpgradeFailed,
                                    35 => V66::HttpProtocolError,
                                    36 => V66::LoopDetected,
                                    37 => V66::ConfigurationError,
                                    n => {
                                        debug_assert_eq!(n, 38, "invalid enum discriminant");
                                        let e66 = {
                                            let l62 = i32::from(*ptr1.add(16).cast::<u8>());
                                            match l62 {
                                                0 => None,
                                                1 => {
                                                    let e = {
                                                        let l63 = *ptr1.add(20).cast::<*mut u8>();
                                                        let l64 = *ptr1.add(24).cast::<usize>();
                                                        let len65 = l64;
                                                        let bytes65 = _rt::Vec::from_raw_parts(
                                                            l63.cast(),
                                                            len65,
                                                            len65,
                                                        );
                                                        _rt::string_lift(bytes65)
                                                    };
                                                    Some(e)
                                                }
                                                _ => _rt::invalid_enum_discriminant(),
                                            }
                                        };
                                        V66::InternalError(e66)
                                    }
                                };
                                v66
                            };
                            Err(e)
                        }
                        _ => _rt::invalid_enum_discriminant(),
                    }
                }
            }
        }
    }
    #[allow(dead_code)]
    pub mod io {
        #[allow(dead_code, clippy::all)]
        pub mod poll {
            #[used]
            #[doc(hidden)]
            static __FORCE_SECTION_REF: fn() =
                super::super::super::__link_custom_section_describing_imports;
            use super::super::super::_rt;
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct Pollable {
                handle: _rt::Resource<Pollable>,
            }
            impl Pollable {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for Pollable {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:io/poll@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]pollable"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            impl Pollable {
                #[allow(unused_unsafe, clippy::all)]
                pub fn ready(&self) -> bool {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/poll@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]pollable.ready"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32);
                        _rt::bool_lift(ret as u8)
                    }
                }
            }
            impl Pollable {
                #[allow(unused_unsafe, clippy::all)]
                pub fn block(&self) {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/poll@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]pollable.block"]
                            fn wit_import(_: i32);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32);
                    }
                }
            }
            #[allow(unused_unsafe, clippy::all)]
            pub fn poll(in_: &[&Pollable]) -> _rt::Vec<u32> {
                unsafe {
                    #[repr(align(4))]
                    struct RetArea([::core::mem::MaybeUninit<u8>; 8]);
                    let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 8]);
                    let vec0 = in_;
                    let len0 = vec0.len();
                    let layout0 = _rt::alloc::Layout::from_size_align_unchecked(vec0.len() * 4, 4);
                    let result0 = if layout0.size() != 0 {
                        let ptr = _rt::alloc::alloc(layout0).cast::<u8>();
                        if ptr.is_null() {
                            _rt::alloc::handle_alloc_error(layout0);
                        }
                        ptr
                    } else {
                        {
                            ::core::ptr::null_mut()
                        }
                    };
                    for (i, e) in vec0.into_iter().enumerate() {
                        let base = result0.add(i * 4);
                        {
                            *base.add(0).cast::<i32>() = (e).handle() as i32;
                        }
                    }
                    let ptr1 = ret_area.0.as_mut_ptr().cast::<u8>();
                    #[cfg(target_arch = "wasm32")]
                    #[link(wasm_import_module = "wasi:io/poll@0.2.0")]
                    extern "C" {
                        #[link_name = "poll"]
                        fn wit_import(_: *mut u8, _: usize, _: *mut u8);
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    fn wit_import(_: *mut u8, _: usize, _: *mut u8) {
                        unreachable!()
                    }
                    wit_import(result0, len0, ptr1);
                    let l2 = *ptr1.add(0).cast::<*mut u8>();
                    let l3 = *ptr1.add(4).cast::<usize>();
                    let len4 = l3;
                    if layout0.size() != 0 {
                        _rt::alloc::dealloc(result0.cast(), layout0);
                    }
                    _rt::Vec::from_raw_parts(l2.cast(), len4, len4)
                }
            }
        }
        #[allow(dead_code, clippy::all)]
        pub mod error {
            #[used]
            #[doc(hidden)]
            static __FORCE_SECTION_REF: fn() =
                super::super::super::__link_custom_section_describing_imports;
            use super::super::super::_rt;
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct Error {
                handle: _rt::Resource<Error>,
            }
            impl Error {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for Error {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:io/error@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]error"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            impl Error {
                #[allow(unused_unsafe, clippy::all)]
                pub fn to_debug_string(&self) -> _rt::String {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 8]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 8]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/error@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]error.to-debug-string"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = *ptr0.add(0).cast::<*mut u8>();
                        let l2 = *ptr0.add(4).cast::<usize>();
                        let len3 = l2;
                        let bytes3 = _rt::Vec::from_raw_parts(l1.cast(), len3, len3);
                        _rt::string_lift(bytes3)
                    }
                }
            }
        }
        #[allow(dead_code, clippy::all)]
        pub mod streams {
            #[used]
            #[doc(hidden)]
            static __FORCE_SECTION_REF: fn() =
                super::super::super::__link_custom_section_describing_imports;
            use super::super::super::_rt;
            pub type Error = super::super::super::wasi::io::error::Error;
            pub type Pollable = super::super::super::wasi::io::poll::Pollable;
            pub enum StreamError {
                LastOperationFailed(Error),
                Closed,
            }
            impl ::core::fmt::Debug for StreamError {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    match self {
                        StreamError::LastOperationFailed(e) => f
                            .debug_tuple("StreamError::LastOperationFailed")
                            .field(e)
                            .finish(),
                        StreamError::Closed => f.debug_tuple("StreamError::Closed").finish(),
                    }
                }
            }
            impl ::core::fmt::Display for StreamError {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    write!(f, "{:?}", self)
                }
            }
            impl std::error::Error for StreamError {}
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct InputStream {
                handle: _rt::Resource<InputStream>,
            }
            impl InputStream {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for InputStream {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]input-stream"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            #[derive(Debug)]
            #[repr(transparent)]
            pub struct OutputStream {
                handle: _rt::Resource<OutputStream>,
            }
            impl OutputStream {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    Self {
                        handle: _rt::Resource::from_handle(handle),
                    }
                }
                #[doc(hidden)]
                pub fn take_handle(&self) -> u32 {
                    _rt::Resource::take_handle(&self.handle)
                }
                #[doc(hidden)]
                pub fn handle(&self) -> u32 {
                    _rt::Resource::handle(&self.handle)
                }
            }
            unsafe impl _rt::WasmResource for OutputStream {
                #[inline]
                unsafe fn drop(_handle: u32) {
                    #[cfg(not(target_arch = "wasm32"))]
                    unreachable!();
                    #[cfg(target_arch = "wasm32")]
                    {
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[resource-drop]output-stream"]
                            fn drop(_: u32);
                        }
                        drop(_handle);
                    }
                }
            }
            impl InputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn read(&self, len: u64) -> Result<_rt::Vec<u8>, StreamError> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]input-stream.read"]
                            fn wit_import(_: i32, _: i64, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i64, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, _rt::as_i64(&len), ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = {
                                    let l2 = *ptr0.add(4).cast::<*mut u8>();
                                    let l3 = *ptr0.add(8).cast::<usize>();
                                    let len4 = l3;
                                    _rt::Vec::from_raw_parts(l2.cast(), len4, len4)
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l5 = i32::from(*ptr0.add(4).cast::<u8>());
                                    let v7 = match l5 {
                                        0 => {
                                            let e7 = {
                                                let l6 = *ptr0.add(8).cast::<i32>();
                                                super::super::super::wasi::io::error::Error::from_handle(
                                                    l6 as u32,
                                                )
                                            };
                                            StreamError::LastOperationFailed(e7)
                                        }
                                        n => {
                                            debug_assert_eq!(n, 1, "invalid enum discriminant");
                                            StreamError::Closed
                                        }
                                    };
                                    v7
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl InputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn blocking_read(&self, len: u64) -> Result<_rt::Vec<u8>, StreamError> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]input-stream.blocking-read"]
                            fn wit_import(_: i32, _: i64, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i64, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, _rt::as_i64(&len), ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = {
                                    let l2 = *ptr0.add(4).cast::<*mut u8>();
                                    let l3 = *ptr0.add(8).cast::<usize>();
                                    let len4 = l3;
                                    _rt::Vec::from_raw_parts(l2.cast(), len4, len4)
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l5 = i32::from(*ptr0.add(4).cast::<u8>());
                                    let v7 = match l5 {
                                        0 => {
                                            let e7 = {
                                                let l6 = *ptr0.add(8).cast::<i32>();
                                                super::super::super::wasi::io::error::Error::from_handle(
                                                    l6 as u32,
                                                )
                                            };
                                            StreamError::LastOperationFailed(e7)
                                        }
                                        n => {
                                            debug_assert_eq!(n, 1, "invalid enum discriminant");
                                            StreamError::Closed
                                        }
                                    };
                                    v7
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl InputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn skip(&self, len: u64) -> Result<u64, StreamError> {
                    unsafe {
                        #[repr(align(8))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 16]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 16]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]input-stream.skip"]
                            fn wit_import(_: i32, _: i64, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i64, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, _rt::as_i64(&len), ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = {
                                    let l2 = *ptr0.add(8).cast::<i64>();
                                    l2 as u64
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l3 = i32::from(*ptr0.add(8).cast::<u8>());
                                    let v5 = match l3 {
                                        0 => {
                                            let e5 = {
                                                let l4 = *ptr0.add(12).cast::<i32>();
                                                super::super::super::wasi::io::error::Error::from_handle(
                                                    l4 as u32,
                                                )
                                            };
                                            StreamError::LastOperationFailed(e5)
                                        }
                                        n => {
                                            debug_assert_eq!(n, 1, "invalid enum discriminant");
                                            StreamError::Closed
                                        }
                                    };
                                    v5
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl InputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn blocking_skip(&self, len: u64) -> Result<u64, StreamError> {
                    unsafe {
                        #[repr(align(8))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 16]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 16]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]input-stream.blocking-skip"]
                            fn wit_import(_: i32, _: i64, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i64, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, _rt::as_i64(&len), ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = {
                                    let l2 = *ptr0.add(8).cast::<i64>();
                                    l2 as u64
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l3 = i32::from(*ptr0.add(8).cast::<u8>());
                                    let v5 = match l3 {
                                        0 => {
                                            let e5 = {
                                                let l4 = *ptr0.add(12).cast::<i32>();
                                                super::super::super::wasi::io::error::Error::from_handle(
                                                    l4 as u32,
                                                )
                                            };
                                            StreamError::LastOperationFailed(e5)
                                        }
                                        n => {
                                            debug_assert_eq!(n, 1, "invalid enum discriminant");
                                            StreamError::Closed
                                        }
                                    };
                                    v5
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl InputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn subscribe(&self) -> Pollable {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]input-stream.subscribe"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32);
                        super::super::super::wasi::io::poll::Pollable::from_handle(ret as u32)
                    }
                }
            }
            impl OutputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn check_write(&self) -> Result<u64, StreamError> {
                    unsafe {
                        #[repr(align(8))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 16]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 16]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]output-stream.check-write"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = {
                                    let l2 = *ptr0.add(8).cast::<i64>();
                                    l2 as u64
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l3 = i32::from(*ptr0.add(8).cast::<u8>());
                                    let v5 = match l3 {
                                        0 => {
                                            let e5 = {
                                                let l4 = *ptr0.add(12).cast::<i32>();
                                                super::super::super::wasi::io::error::Error::from_handle(
                                                    l4 as u32,
                                                )
                                            };
                                            StreamError::LastOperationFailed(e5)
                                        }
                                        n => {
                                            debug_assert_eq!(n, 1, "invalid enum discriminant");
                                            StreamError::Closed
                                        }
                                    };
                                    v5
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn write(&self, contents: &[u8]) -> Result<(), StreamError> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let vec0 = contents;
                        let ptr0 = vec0.as_ptr().cast::<u8>();
                        let len0 = vec0.len();
                        let ptr1 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]output-stream.write"]
                            fn wit_import(_: i32, _: *mut u8, _: usize, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8, _: usize, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0.cast_mut(), len0, ptr1);
                        let l2 = i32::from(*ptr1.add(0).cast::<u8>());
                        match l2 {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l3 = i32::from(*ptr1.add(4).cast::<u8>());
                                    let v5 = match l3 {
                                        0 => {
                                            let e5 = {
                                                let l4 = *ptr1.add(8).cast::<i32>();
                                                super::super::super::wasi::io::error::Error::from_handle(
                                                    l4 as u32,
                                                )
                                            };
                                            StreamError::LastOperationFailed(e5)
                                        }
                                        n => {
                                            debug_assert_eq!(n, 1, "invalid enum discriminant");
                                            StreamError::Closed
                                        }
                                    };
                                    v5
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn blocking_write_and_flush(&self, contents: &[u8]) -> Result<(), StreamError> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let vec0 = contents;
                        let ptr0 = vec0.as_ptr().cast::<u8>();
                        let len0 = vec0.len();
                        let ptr1 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]output-stream.blocking-write-and-flush"]
                            fn wit_import(_: i32, _: *mut u8, _: usize, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8, _: usize, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0.cast_mut(), len0, ptr1);
                        let l2 = i32::from(*ptr1.add(0).cast::<u8>());
                        match l2 {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l3 = i32::from(*ptr1.add(4).cast::<u8>());
                                    let v5 = match l3 {
                                        0 => {
                                            let e5 = {
                                                let l4 = *ptr1.add(8).cast::<i32>();
                                                super::super::super::wasi::io::error::Error::from_handle(
                                                    l4 as u32,
                                                )
                                            };
                                            StreamError::LastOperationFailed(e5)
                                        }
                                        n => {
                                            debug_assert_eq!(n, 1, "invalid enum discriminant");
                                            StreamError::Closed
                                        }
                                    };
                                    v5
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn flush(&self) -> Result<(), StreamError> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]output-stream.flush"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l2 = i32::from(*ptr0.add(4).cast::<u8>());
                                    let v4 = match l2 {
                                        0 => {
                                            let e4 = {
                                                let l3 = *ptr0.add(8).cast::<i32>();
                                                super::super::super::wasi::io::error::Error::from_handle(
                                                    l3 as u32,
                                                )
                                            };
                                            StreamError::LastOperationFailed(e4)
                                        }
                                        n => {
                                            debug_assert_eq!(n, 1, "invalid enum discriminant");
                                            StreamError::Closed
                                        }
                                    };
                                    v4
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn blocking_flush(&self) -> Result<(), StreamError> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]output-stream.blocking-flush"]
                            fn wit_import(_: i32, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l2 = i32::from(*ptr0.add(4).cast::<u8>());
                                    let v4 = match l2 {
                                        0 => {
                                            let e4 = {
                                                let l3 = *ptr0.add(8).cast::<i32>();
                                                super::super::super::wasi::io::error::Error::from_handle(
                                                    l3 as u32,
                                                )
                                            };
                                            StreamError::LastOperationFailed(e4)
                                        }
                                        n => {
                                            debug_assert_eq!(n, 1, "invalid enum discriminant");
                                            StreamError::Closed
                                        }
                                    };
                                    v4
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn subscribe(&self) -> Pollable {
                    unsafe {
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]output-stream.subscribe"]
                            fn wit_import(_: i32) -> i32;
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32) -> i32 {
                            unreachable!()
                        }
                        let ret = wit_import((self).handle() as i32);
                        super::super::super::wasi::io::poll::Pollable::from_handle(ret as u32)
                    }
                }
            }
            impl OutputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn write_zeroes(&self, len: u64) -> Result<(), StreamError> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]output-stream.write-zeroes"]
                            fn wit_import(_: i32, _: i64, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i64, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, _rt::as_i64(&len), ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l2 = i32::from(*ptr0.add(4).cast::<u8>());
                                    let v4 = match l2 {
                                        0 => {
                                            let e4 = {
                                                let l3 = *ptr0.add(8).cast::<i32>();
                                                super::super::super::wasi::io::error::Error::from_handle(
                                                    l3 as u32,
                                                )
                                            };
                                            StreamError::LastOperationFailed(e4)
                                        }
                                        n => {
                                            debug_assert_eq!(n, 1, "invalid enum discriminant");
                                            StreamError::Closed
                                        }
                                    };
                                    v4
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn blocking_write_zeroes_and_flush(&self, len: u64) -> Result<(), StreamError> {
                    unsafe {
                        #[repr(align(4))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 12]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 12]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]output-stream.blocking-write-zeroes-and-flush"]
                            fn wit_import(_: i32, _: i64, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i64, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import((self).handle() as i32, _rt::as_i64(&len), ptr0);
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = ();
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l2 = i32::from(*ptr0.add(4).cast::<u8>());
                                    let v4 = match l2 {
                                        0 => {
                                            let e4 = {
                                                let l3 = *ptr0.add(8).cast::<i32>();
                                                super::super::super::wasi::io::error::Error::from_handle(
                                                    l3 as u32,
                                                )
                                            };
                                            StreamError::LastOperationFailed(e4)
                                        }
                                        n => {
                                            debug_assert_eq!(n, 1, "invalid enum discriminant");
                                            StreamError::Closed
                                        }
                                    };
                                    v4
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn splice(&self, src: &InputStream, len: u64) -> Result<u64, StreamError> {
                    unsafe {
                        #[repr(align(8))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 16]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 16]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]output-stream.splice"]
                            fn wit_import(_: i32, _: i32, _: i64, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i32, _: i64, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import(
                            (self).handle() as i32,
                            (src).handle() as i32,
                            _rt::as_i64(&len),
                            ptr0,
                        );
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = {
                                    let l2 = *ptr0.add(8).cast::<i64>();
                                    l2 as u64
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l3 = i32::from(*ptr0.add(8).cast::<u8>());
                                    let v5 = match l3 {
                                        0 => {
                                            let e5 = {
                                                let l4 = *ptr0.add(12).cast::<i32>();
                                                super::super::super::wasi::io::error::Error::from_handle(
                                                    l4 as u32,
                                                )
                                            };
                                            StreamError::LastOperationFailed(e5)
                                        }
                                        n => {
                                            debug_assert_eq!(n, 1, "invalid enum discriminant");
                                            StreamError::Closed
                                        }
                                    };
                                    v5
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
            impl OutputStream {
                #[allow(unused_unsafe, clippy::all)]
                pub fn blocking_splice(
                    &self,
                    src: &InputStream,
                    len: u64,
                ) -> Result<u64, StreamError> {
                    unsafe {
                        #[repr(align(8))]
                        struct RetArea([::core::mem::MaybeUninit<u8>; 16]);
                        let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 16]);
                        let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                        #[cfg(target_arch = "wasm32")]
                        #[link(wasm_import_module = "wasi:io/streams@0.2.0")]
                        extern "C" {
                            #[link_name = "[method]output-stream.blocking-splice"]
                            fn wit_import(_: i32, _: i32, _: i64, _: *mut u8);
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fn wit_import(_: i32, _: i32, _: i64, _: *mut u8) {
                            unreachable!()
                        }
                        wit_import(
                            (self).handle() as i32,
                            (src).handle() as i32,
                            _rt::as_i64(&len),
                            ptr0,
                        );
                        let l1 = i32::from(*ptr0.add(0).cast::<u8>());
                        match l1 {
                            0 => {
                                let e = {
                                    let l2 = *ptr0.add(8).cast::<i64>();
                                    l2 as u64
                                };
                                Ok(e)
                            }
                            1 => {
                                let e = {
                                    let l3 = i32::from(*ptr0.add(8).cast::<u8>());
                                    let v5 = match l3 {
                                        0 => {
                                            let e5 = {
                                                let l4 = *ptr0.add(12).cast::<i32>();
                                                super::super::super::wasi::io::error::Error::from_handle(
                                                    l4 as u32,
                                                )
                                            };
                                            StreamError::LastOperationFailed(e5)
                                        }
                                        n => {
                                            debug_assert_eq!(n, 1, "invalid enum discriminant");
                                            StreamError::Closed
                                        }
                                    };
                                    v5
                                };
                                Err(e)
                            }
                            _ => _rt::invalid_enum_discriminant(),
                        }
                    }
                }
            }
        }
    }
}
mod _rt {
    use core::fmt;
    use core::marker;
    use core::sync::atomic::{AtomicU32, Ordering::Relaxed};
    /// A type which represents a component model resource, either imported or
    /// exported into this component.
    ///
    /// This is a low-level wrapper which handles the lifetime of the resource
    /// (namely this has a destructor). The `T` provided defines the component model
    /// intrinsics that this wrapper uses.
    ///
    /// One of the chief purposes of this type is to provide `Deref` implementations
    /// to access the underlying data when it is owned.
    ///
    /// This type is primarily used in generated code for exported and imported
    /// resources.
    #[repr(transparent)]
    pub struct Resource<T: WasmResource> {
        handle: AtomicU32,
        _marker: marker::PhantomData<T>,
    }
    /// A trait which all wasm resources implement, namely providing the ability to
    /// drop a resource.
    ///
    /// This generally is implemented by generated code, not user-facing code.
    #[allow(clippy::missing_safety_doc)]
    pub unsafe trait WasmResource {
        /// Invokes the `[resource-drop]...` intrinsic.
        unsafe fn drop(handle: u32);
    }
    impl<T: WasmResource> Resource<T> {
        #[doc(hidden)]
        pub unsafe fn from_handle(handle: u32) -> Self {
            debug_assert!(handle != u32::MAX);
            Self {
                handle: AtomicU32::new(handle),
                _marker: marker::PhantomData,
            }
        }
        /// Takes ownership of the handle owned by `resource`.
        ///
        /// Note that this ideally would be `into_handle` taking `Resource<T>` by
        /// ownership. The code generator does not enable that in all situations,
        /// unfortunately, so this is provided instead.
        ///
        /// Also note that `take_handle` is in theory only ever called on values
        /// owned by a generated function. For example a generated function might
        /// take `Resource<T>` as an argument but then call `take_handle` on a
        /// reference to that argument. In that sense the dynamic nature of
        /// `take_handle` should only be exposed internally to generated code, not
        /// to user code.
        #[doc(hidden)]
        pub fn take_handle(resource: &Resource<T>) -> u32 {
            resource.handle.swap(u32::MAX, Relaxed)
        }
        #[doc(hidden)]
        pub fn handle(resource: &Resource<T>) -> u32 {
            resource.handle.load(Relaxed)
        }
    }
    impl<T: WasmResource> fmt::Debug for Resource<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Resource")
                .field("handle", &self.handle)
                .finish()
        }
    }
    impl<T: WasmResource> Drop for Resource<T> {
        fn drop(&mut self) {
            unsafe {
                match self.handle.load(Relaxed) {
                    u32::MAX => {}
                    other => T::drop(other),
                }
            }
        }
    }
    pub unsafe fn bool_lift(val: u8) -> bool {
        if cfg!(debug_assertions) {
            match val {
                0 => false,
                1 => true,
                _ => panic!("invalid bool discriminant"),
            }
        } else {
            val != 0
        }
    }
    pub use alloc_crate::alloc;
    pub use alloc_crate::vec::Vec;
    pub fn as_i64<T: AsI64>(t: T) -> i64 {
        t.as_i64()
    }
    pub trait AsI64 {
        fn as_i64(self) -> i64;
    }
    impl<'a, T: Copy + AsI64> AsI64 for &'a T {
        fn as_i64(self) -> i64 {
            (*self).as_i64()
        }
    }
    impl AsI64 for i64 {
        #[inline]
        fn as_i64(self) -> i64 {
            self as i64
        }
    }
    impl AsI64 for u64 {
        #[inline]
        fn as_i64(self) -> i64 {
            self as i64
        }
    }
    pub use alloc_crate::string::String;
    pub unsafe fn string_lift(bytes: Vec<u8>) -> String {
        if cfg!(debug_assertions) {
            String::from_utf8(bytes).unwrap()
        } else {
            String::from_utf8_unchecked(bytes)
        }
    }
    pub unsafe fn invalid_enum_discriminant<T>() -> T {
        if cfg!(debug_assertions) {
            panic!("invalid enum discriminant")
        } else {
            core::hint::unreachable_unchecked()
        }
    }
    pub unsafe fn cabi_dealloc(ptr: *mut u8, size: usize, align: usize) {
        if size == 0 {
            return;
        }
        let layout = alloc::Layout::from_size_align_unchecked(size, align);
        alloc::dealloc(ptr, layout);
    }
    pub fn as_i32<T: AsI32>(t: T) -> i32 {
        t.as_i32()
    }
    pub trait AsI32 {
        fn as_i32(self) -> i32;
    }
    impl<'a, T: Copy + AsI32> AsI32 for &'a T {
        fn as_i32(self) -> i32 {
            (*self).as_i32()
        }
    }
    impl AsI32 for i32 {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for u32 {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for i16 {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for u16 {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for i8 {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for u8 {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for char {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for usize {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub fn run_ctors_once() {
        wit_bindgen_rt::run_ctors_once();
    }
    extern crate alloc as alloc_crate;
}
/// Generates `#[no_mangle]` functions to export the specified type as the
/// root implementation of all generated traits.
///
/// For more information see the documentation of `wit_bindgen::generate!`.
///
/// ```rust
/// # macro_rules! export{ ($($t:tt)*) => (); }
/// # trait Guest {}
/// struct MyType;
///
/// impl Guest for MyType {
///     // ...
/// }
///
/// export!(MyType);
/// ```
#[allow(unused_macros)]
#[doc(hidden)]
macro_rules! __export_task_queue_impl {
    ($ty:ident) => {
        self::export!($ty with_types_in self);
    };
    ($ty:ident with_types_in $($path_to_types_root:tt)*) => {
        $($path_to_types_root)*:: __export_world_task_queue_cabi!($ty with_types_in
        $($path_to_types_root)*);
    };
}
#[doc(inline)]
pub(crate) use __export_task_queue_impl as export;
#[cfg(target_arch = "wasm32")]
#[link_section = "component-type:wit-bindgen:0.30.0:task-queue:encoded world"]
#[doc(hidden)]
pub static __WIT_BINDGEN_COMPONENT_TYPE: [u8; 6655] = *b"\
\0asm\x0d\0\x01\0\0\x19\x16wit-component-encoding\x04\0\x07\xfe2\x01A\x02\x01A\x1d\
\x01B\x0a\x04\0\x08pollable\x03\x01\x01h\0\x01@\x01\x04self\x01\0\x7f\x04\0\x16[\
method]pollable.ready\x01\x02\x01@\x01\x04self\x01\x01\0\x04\0\x16[method]pollab\
le.block\x01\x03\x01p\x01\x01py\x01@\x01\x02in\x04\0\x05\x04\0\x04poll\x01\x06\x03\
\x01\x12wasi:io/poll@0.2.0\x05\0\x02\x03\0\0\x08pollable\x01B\x0f\x02\x03\x02\x01\
\x01\x04\0\x08pollable\x03\0\0\x01w\x04\0\x07instant\x03\0\x02\x01w\x04\0\x08dur\
ation\x03\0\x04\x01@\0\0\x03\x04\0\x03now\x01\x06\x01@\0\0\x05\x04\0\x0aresoluti\
on\x01\x07\x01i\x01\x01@\x01\x04when\x03\0\x08\x04\0\x11subscribe-instant\x01\x09\
\x01@\x01\x04when\x05\0\x08\x04\0\x12subscribe-duration\x01\x0a\x03\x01!wasi:clo\
cks/monotonic-clock@0.2.0\x05\x02\x01B\x04\x04\0\x05error\x03\x01\x01h\0\x01@\x01\
\x04self\x01\0s\x04\0\x1d[method]error.to-debug-string\x01\x02\x03\x01\x13wasi:i\
o/error@0.2.0\x05\x03\x02\x03\0\x02\x05error\x01B(\x02\x03\x02\x01\x04\x04\0\x05\
error\x03\0\0\x02\x03\x02\x01\x01\x04\0\x08pollable\x03\0\x02\x01i\x01\x01q\x02\x15\
last-operation-failed\x01\x04\0\x06closed\0\0\x04\0\x0cstream-error\x03\0\x05\x04\
\0\x0cinput-stream\x03\x01\x04\0\x0doutput-stream\x03\x01\x01h\x07\x01p}\x01j\x01\
\x0a\x01\x06\x01@\x02\x04self\x09\x03lenw\0\x0b\x04\0\x19[method]input-stream.re\
ad\x01\x0c\x04\0\"[method]input-stream.blocking-read\x01\x0c\x01j\x01w\x01\x06\x01\
@\x02\x04self\x09\x03lenw\0\x0d\x04\0\x19[method]input-stream.skip\x01\x0e\x04\0\
\"[method]input-stream.blocking-skip\x01\x0e\x01i\x03\x01@\x01\x04self\x09\0\x0f\
\x04\0\x1e[method]input-stream.subscribe\x01\x10\x01h\x08\x01@\x01\x04self\x11\0\
\x0d\x04\0![method]output-stream.check-write\x01\x12\x01j\0\x01\x06\x01@\x02\x04\
self\x11\x08contents\x0a\0\x13\x04\0\x1b[method]output-stream.write\x01\x14\x04\0\
.[method]output-stream.blocking-write-and-flush\x01\x14\x01@\x01\x04self\x11\0\x13\
\x04\0\x1b[method]output-stream.flush\x01\x15\x04\0$[method]output-stream.blocki\
ng-flush\x01\x15\x01@\x01\x04self\x11\0\x0f\x04\0\x1f[method]output-stream.subsc\
ribe\x01\x16\x01@\x02\x04self\x11\x03lenw\0\x13\x04\0\"[method]output-stream.wri\
te-zeroes\x01\x17\x04\05[method]output-stream.blocking-write-zeroes-and-flush\x01\
\x17\x01@\x03\x04self\x11\x03src\x09\x03lenw\0\x0d\x04\0\x1c[method]output-strea\
m.splice\x01\x18\x04\0%[method]output-stream.blocking-splice\x01\x18\x03\x01\x15\
wasi:io/streams@0.2.0\x05\x05\x02\x03\0\x01\x08duration\x02\x03\0\x03\x0cinput-s\
tream\x02\x03\0\x03\x0doutput-stream\x01B\xc0\x01\x02\x03\x02\x01\x06\x04\0\x08d\
uration\x03\0\0\x02\x03\x02\x01\x07\x04\0\x0cinput-stream\x03\0\x02\x02\x03\x02\x01\
\x08\x04\0\x0doutput-stream\x03\0\x04\x02\x03\x02\x01\x04\x04\0\x08io-error\x03\0\
\x06\x02\x03\x02\x01\x01\x04\0\x08pollable\x03\0\x08\x01q\x0a\x03get\0\0\x04head\
\0\0\x04post\0\0\x03put\0\0\x06delete\0\0\x07connect\0\0\x07options\0\0\x05trace\
\0\0\x05patch\0\0\x05other\x01s\0\x04\0\x06method\x03\0\x0a\x01q\x03\x04HTTP\0\0\
\x05HTTPS\0\0\x05other\x01s\0\x04\0\x06scheme\x03\0\x0c\x01ks\x01k{\x01r\x02\x05\
rcode\x0e\x09info-code\x0f\x04\0\x11DNS-error-payload\x03\0\x10\x01k}\x01r\x02\x08\
alert-id\x12\x0dalert-message\x0e\x04\0\x1aTLS-alert-received-payload\x03\0\x13\x01\
ky\x01r\x02\x0afield-name\x0e\x0afield-size\x15\x04\0\x12field-size-payload\x03\0\
\x16\x01kw\x01k\x17\x01q'\x0bDNS-timeout\0\0\x09DNS-error\x01\x11\0\x15destinati\
on-not-found\0\0\x17destination-unavailable\0\0\x19destination-IP-prohibited\0\0\
\x19destination-IP-unroutable\0\0\x12connection-refused\0\0\x15connection-termin\
ated\0\0\x12connection-timeout\0\0\x17connection-read-timeout\0\0\x18connection-\
write-timeout\0\0\x18connection-limit-reached\0\0\x12TLS-protocol-error\0\0\x15T\
LS-certificate-error\0\0\x12TLS-alert-received\x01\x14\0\x13HTTP-request-denied\0\
\0\x1cHTTP-request-length-required\0\0\x16HTTP-request-body-size\x01\x18\0\x1bHT\
TP-request-method-invalid\0\0\x18HTTP-request-URI-invalid\0\0\x19HTTP-request-UR\
I-too-long\0\0\x20HTTP-request-header-section-size\x01\x15\0\x18HTTP-request-hea\
der-size\x01\x19\0!HTTP-request-trailer-section-size\x01\x15\0\x19HTTP-request-t\
railer-size\x01\x17\0\x18HTTP-response-incomplete\0\0!HTTP-response-header-secti\
on-size\x01\x15\0\x19HTTP-response-header-size\x01\x17\0\x17HTTP-response-body-s\
ize\x01\x18\0\"HTTP-response-trailer-section-size\x01\x15\0\x1aHTTP-response-tra\
iler-size\x01\x17\0\x1dHTTP-response-transfer-coding\x01\x0e\0\x1cHTTP-response-\
content-coding\x01\x0e\0\x15HTTP-response-timeout\0\0\x13HTTP-upgrade-failed\0\0\
\x13HTTP-protocol-error\0\0\x0dloop-detected\0\0\x13configuration-error\0\0\x0ei\
nternal-error\x01\x0e\0\x04\0\x0aerror-code\x03\0\x1a\x01q\x03\x0einvalid-syntax\
\0\0\x09forbidden\0\0\x09immutable\0\0\x04\0\x0cheader-error\x03\0\x1c\x01s\x04\0\
\x09field-key\x03\0\x1e\x01p}\x04\0\x0bfield-value\x03\0\x20\x04\0\x06fields\x03\
\x01\x04\0\x07headers\x03\0\"\x04\0\x08trailers\x03\0\"\x04\0\x10incoming-reques\
t\x03\x01\x04\0\x10outgoing-request\x03\x01\x04\0\x0frequest-options\x03\x01\x04\
\0\x11response-outparam\x03\x01\x01{\x04\0\x0bstatus-code\x03\0)\x04\0\x11incomi\
ng-response\x03\x01\x04\0\x0dincoming-body\x03\x01\x04\0\x0ffuture-trailers\x03\x01\
\x04\0\x11outgoing-response\x03\x01\x04\0\x0doutgoing-body\x03\x01\x04\0\x18futu\
re-incoming-response\x03\x01\x01i\"\x01@\0\01\x04\0\x13[constructor]fields\x012\x01\
o\x02\x1f!\x01p3\x01j\x011\x01\x1d\x01@\x01\x07entries4\05\x04\0\x18[static]fiel\
ds.from-list\x016\x01h\"\x01p!\x01@\x02\x04self7\x04name\x1f\08\x04\0\x12[method\
]fields.get\x019\x01@\x02\x04self7\x04name\x1f\0\x7f\x04\0\x12[method]fields.has\
\x01:\x01j\0\x01\x1d\x01@\x03\x04self7\x04name\x1f\x05value8\0;\x04\0\x12[method\
]fields.set\x01<\x01@\x02\x04self7\x04name\x1f\0;\x04\0\x15[method]fields.delete\
\x01=\x01@\x03\x04self7\x04name\x1f\x05value!\0;\x04\0\x15[method]fields.append\x01\
>\x01@\x01\x04self7\04\x04\0\x16[method]fields.entries\x01?\x01@\x01\x04self7\01\
\x04\0\x14[method]fields.clone\x01@\x01h%\x01@\x01\x04self\xc1\0\0\x0b\x04\0\x1f\
[method]incoming-request.method\x01B\x01@\x01\x04self\xc1\0\0\x0e\x04\0([method]\
incoming-request.path-with-query\x01C\x01k\x0d\x01@\x01\x04self\xc1\0\0\xc4\0\x04\
\0\x1f[method]incoming-request.scheme\x01E\x04\0\"[method]incoming-request.autho\
rity\x01C\x01i#\x01@\x01\x04self\xc1\0\0\xc6\0\x04\0\x20[method]incoming-request\
.headers\x01G\x01i,\x01j\x01\xc8\0\0\x01@\x01\x04self\xc1\0\0\xc9\0\x04\0\x20[me\
thod]incoming-request.consume\x01J\x01i&\x01@\x01\x07headers\xc6\0\0\xcb\0\x04\0\
\x1d[constructor]outgoing-request\x01L\x01h&\x01i/\x01j\x01\xce\0\0\x01@\x01\x04\
self\xcd\0\0\xcf\0\x04\0\x1d[method]outgoing-request.body\x01P\x01@\x01\x04self\xcd\
\0\0\x0b\x04\0\x1f[method]outgoing-request.method\x01Q\x01j\0\0\x01@\x02\x04self\
\xcd\0\x06method\x0b\0\xd2\0\x04\0#[method]outgoing-request.set-method\x01S\x01@\
\x01\x04self\xcd\0\0\x0e\x04\0([method]outgoing-request.path-with-query\x01T\x01\
@\x02\x04self\xcd\0\x0fpath-with-query\x0e\0\xd2\0\x04\0,[method]outgoing-reques\
t.set-path-with-query\x01U\x01@\x01\x04self\xcd\0\0\xc4\0\x04\0\x1f[method]outgo\
ing-request.scheme\x01V\x01@\x02\x04self\xcd\0\x06scheme\xc4\0\0\xd2\0\x04\0#[me\
thod]outgoing-request.set-scheme\x01W\x04\0\"[method]outgoing-request.authority\x01\
T\x01@\x02\x04self\xcd\0\x09authority\x0e\0\xd2\0\x04\0&[method]outgoing-request\
.set-authority\x01X\x01@\x01\x04self\xcd\0\0\xc6\0\x04\0\x20[method]outgoing-req\
uest.headers\x01Y\x01i'\x01@\0\0\xda\0\x04\0\x1c[constructor]request-options\x01\
[\x01h'\x01k\x01\x01@\x01\x04self\xdc\0\0\xdd\0\x04\0'[method]request-options.co\
nnect-timeout\x01^\x01@\x02\x04self\xdc\0\x08duration\xdd\0\0\xd2\0\x04\0+[metho\
d]request-options.set-connect-timeout\x01_\x04\0*[method]request-options.first-b\
yte-timeout\x01^\x04\0.[method]request-options.set-first-byte-timeout\x01_\x04\0\
-[method]request-options.between-bytes-timeout\x01^\x04\01[method]request-option\
s.set-between-bytes-timeout\x01_\x01i(\x01i.\x01j\x01\xe1\0\x01\x1b\x01@\x02\x05\
param\xe0\0\x08response\xe2\0\x01\0\x04\0\x1d[static]response-outparam.set\x01c\x01\
h+\x01@\x01\x04self\xe4\0\0*\x04\0\x20[method]incoming-response.status\x01e\x01@\
\x01\x04self\xe4\0\0\xc6\0\x04\0![method]incoming-response.headers\x01f\x01@\x01\
\x04self\xe4\0\0\xc9\0\x04\0![method]incoming-response.consume\x01g\x01h,\x01i\x03\
\x01j\x01\xe9\0\0\x01@\x01\x04self\xe8\0\0\xea\0\x04\0\x1c[method]incoming-body.\
stream\x01k\x01i-\x01@\x01\x04this\xc8\0\0\xec\0\x04\0\x1c[static]incoming-body.\
finish\x01m\x01h-\x01i\x09\x01@\x01\x04self\xee\0\0\xef\0\x04\0![method]future-t\
railers.subscribe\x01p\x01i$\x01k\xf1\0\x01j\x01\xf2\0\x01\x1b\x01j\x01\xf3\0\0\x01\
k\xf4\0\x01@\x01\x04self\xee\0\0\xf5\0\x04\0\x1b[method]future-trailers.get\x01v\
\x01@\x01\x07headers\xc6\0\0\xe1\0\x04\0\x1e[constructor]outgoing-response\x01w\x01\
h.\x01@\x01\x04self\xf8\0\0*\x04\0%[method]outgoing-response.status-code\x01y\x01\
@\x02\x04self\xf8\0\x0bstatus-code*\0\xd2\0\x04\0)[method]outgoing-response.set-\
status-code\x01z\x01@\x01\x04self\xf8\0\0\xc6\0\x04\0![method]outgoing-response.\
headers\x01{\x01@\x01\x04self\xf8\0\0\xcf\0\x04\0\x1e[method]outgoing-response.b\
ody\x01|\x01h/\x01i\x05\x01j\x01\xfe\0\0\x01@\x01\x04self\xfd\0\0\xff\0\x04\0\x1b\
[method]outgoing-body.write\x01\x80\x01\x01j\0\x01\x1b\x01@\x02\x04this\xce\0\x08\
trailers\xf2\0\0\x81\x01\x04\0\x1c[static]outgoing-body.finish\x01\x82\x01\x01h0\
\x01@\x01\x04self\x83\x01\0\xef\0\x04\0*[method]future-incoming-response.subscri\
be\x01\x84\x01\x01i+\x01j\x01\x85\x01\x01\x1b\x01j\x01\x86\x01\0\x01k\x87\x01\x01\
@\x01\x04self\x83\x01\0\x88\x01\x04\0$[method]future-incoming-response.get\x01\x89\
\x01\x01h\x07\x01k\x1b\x01@\x01\x03err\x8a\x01\0\x8b\x01\x04\0\x0fhttp-error-cod\
e\x01\x8c\x01\x03\x01\x15wasi:http/types@0.2.0\x05\x09\x02\x03\0\x04\x10outgoing\
-request\x02\x03\0\x04\x0frequest-options\x02\x03\0\x04\x18future-incoming-respo\
nse\x02\x03\0\x04\x0aerror-code\x01B\x0f\x02\x03\x02\x01\x0a\x04\0\x10outgoing-r\
equest\x03\0\0\x02\x03\x02\x01\x0b\x04\0\x0frequest-options\x03\0\x02\x02\x03\x02\
\x01\x0c\x04\0\x18future-incoming-response\x03\0\x04\x02\x03\x02\x01\x0d\x04\0\x0a\
error-code\x03\0\x06\x01i\x01\x01i\x03\x01k\x09\x01i\x05\x01j\x01\x0b\x01\x07\x01\
@\x02\x07request\x08\x07options\x0a\0\x0c\x04\0\x06handle\x01\x0d\x03\x01\x20was\
i:http/outgoing-handler@0.2.0\x05\x0e\x01B\x08\x01p}\x04\0\x0fserialized-json\x03\
\0\0\x01r\x02\x09timestampw\x07request\x01\x04\0\x10task-queue-input\x03\0\x02\x01\
s\x04\0\x05error\x03\0\x04\x01j\x01\x01\x01\x05\x04\0\x06output\x03\0\x06\x03\x01\
\x15lay3r:avs/types@0.3.0\x05\x0f\x02\x03\0\x06\x10task-queue-input\x03\0\x10tas\
k-queue-input\x03\0\x10\x02\x03\0\x06\x06output\x03\0\x06output\x03\0\x12\x01@\x01\
\x07request\x11\0\x13\x04\0\x08run-task\x01\x14\x04\x01\x1alay3r:avs/task-queue@\
0.3.0\x04\0\x0b\x10\x01\0\x0atask-queue\x03\0\0\0G\x09producers\x01\x0cprocessed\
-by\x02\x0dwit-component\x070.215.0\x10wit-bindgen-rust\x060.30.0";
#[inline(never)]
#[doc(hidden)]
pub fn __link_custom_section_describing_imports() {
    wit_bindgen_rt::maybe_link_cabi_realloc();
}
