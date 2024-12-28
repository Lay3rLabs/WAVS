pub type EthLog = lay3r::avs::eth_event_types::EthLog;
#[doc(hidden)]
#[allow(non_snake_case)]
pub unsafe fn _export_process_eth_event_cabi<T: Guest>(
    arg0: *mut u8,
    arg1: usize,
    arg2: *mut u8,
    arg3: usize,
    arg4: *mut u8,
    arg5: usize,
) -> *mut u8 {
    #[cfg(target_arch = "wasm32")]
    _rt::run_ctors_once();
    let len0 = arg1;
    let base4 = arg2;
    let len4 = arg3;
    let mut result4 = _rt::Vec::with_capacity(len4);
    for i in 0..len4 {
        let base = base4.add(i * 8);
        let e4 = {
            let l1 = *base.add(0).cast::<*mut u8>();
            let l2 = *base.add(4).cast::<usize>();
            let len3 = l2;
            _rt::Vec::from_raw_parts(l1.cast(), len3, len3)
        };
        result4.push(e4);
    }
    _rt::cabi_dealloc(base4, len4 * 8, 4);
    let len5 = arg5;
    let result6 = T::process_eth_event(lay3r::avs::eth_event_types::EthLog {
        address: _rt::Vec::from_raw_parts(arg0.cast(), len0, len0),
        log_topics: result4,
        log_data: _rt::Vec::from_raw_parts(arg4.cast(), len5, len5),
    });
    let ptr7 = _RET_AREA.0.as_mut_ptr().cast::<u8>();
    match result6 {
        Ok(e) => {
            *ptr7.add(0).cast::<u8>() = (0i32) as u8;
            let vec8 = (e).into_boxed_slice();
            let ptr8 = vec8.as_ptr().cast::<u8>();
            let len8 = vec8.len();
            ::core::mem::forget(vec8);
            *ptr7.add(8).cast::<usize>() = len8;
            *ptr7.add(4).cast::<*mut u8>() = ptr8.cast_mut();
        }
        Err(e) => {
            *ptr7.add(0).cast::<u8>() = (1i32) as u8;
            let vec9 = (e.into_bytes()).into_boxed_slice();
            let ptr9 = vec9.as_ptr().cast::<u8>();
            let len9 = vec9.len();
            ::core::mem::forget(vec9);
            *ptr7.add(8).cast::<usize>() = len9;
            *ptr7.add(4).cast::<*mut u8>() = ptr9.cast_mut();
        }
    };
    ptr7
}
#[doc(hidden)]
#[allow(non_snake_case)]
pub unsafe fn __post_return_process_eth_event<T: Guest>(arg0: *mut u8) {
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
    fn process_eth_event(log: EthLog) -> Result<_rt::Vec<u8>, _rt::String>;
}
#[doc(hidden)]
macro_rules! __export_world_eth_event_world_cabi {
    ($ty:ident with_types_in $($path_to_types:tt)*) => {
        const _ : () = { #[export_name = "process-eth-event"] unsafe extern "C" fn
        export_process_eth_event(arg0 : * mut u8, arg1 : usize, arg2 : * mut u8, arg3 :
        usize, arg4 : * mut u8, arg5 : usize,) -> * mut u8 { $($path_to_types)*::
        _export_process_eth_event_cabi::<$ty > (arg0, arg1, arg2, arg3, arg4, arg5) }
        #[export_name = "cabi_post_process-eth-event"] unsafe extern "C" fn
        _post_return_process_eth_event(arg0 : * mut u8,) { $($path_to_types)*::
        __post_return_process_eth_event::<$ty > (arg0) } };
    };
}
#[doc(hidden)]
pub(crate) use __export_world_eth_event_world_cabi;
#[repr(align(4))]
struct _RetArea([::core::mem::MaybeUninit<u8>; 12]);
static mut _RET_AREA: _RetArea = _RetArea([::core::mem::MaybeUninit::uninit(); 12]);
#[allow(dead_code)]
pub mod lay3r {
    #[allow(dead_code)]
    pub mod avs {
        #[allow(dead_code, clippy::all)]
        pub mod eth_event_types {
            #[used]
            #[doc(hidden)]
            static __FORCE_SECTION_REF: fn() =
                super::super::super::__link_custom_section_describing_imports;
            use super::super::super::_rt;
            /// An alloy log can be recreated with the info here
            /// block height is extra
            #[derive(Clone)]
            pub struct EthLog {
                /// the address that emitted an event
                pub address: _rt::Vec<u8>,
                /// the raw log topics that can be decoded into an event
                pub log_topics: _rt::Vec<_rt::Vec<u8>>,
                /// the raw log data that can be decoded into an event
                pub log_data: _rt::Vec<u8>,
            }
            impl ::core::fmt::Debug for EthLog {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct("EthLog")
                        .field("address", &self.address)
                        .field("log-topics", &self.log_topics)
                        .field("log-data", &self.log_data)
                        .finish()
                }
            }
        }
    }
}
mod _rt {
    pub use alloc_crate::vec::Vec;
    #[cfg(target_arch = "wasm32")]
    pub fn run_ctors_once() {
        wit_bindgen_rt::run_ctors_once();
    }
    pub unsafe fn cabi_dealloc(ptr: *mut u8, size: usize, align: usize) {
        if size == 0 {
            return;
        }
        let layout = alloc::Layout::from_size_align_unchecked(size, align);
        alloc::dealloc(ptr, layout);
    }
    pub use alloc_crate::string::String;
    extern crate alloc as alloc_crate;
    pub use alloc_crate::alloc;
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
macro_rules! __export_eth_event_world_impl {
    ($ty:ident) => {
        self::export!($ty with_types_in self);
    };
    ($ty:ident with_types_in $($path_to_types_root:tt)*) => {
        $($path_to_types_root)*:: __export_world_eth_event_world_cabi!($ty with_types_in
        $($path_to_types_root)*);
    };
}
#[doc(inline)]
pub(crate) use __export_eth_event_world_impl as export;
#[cfg(target_arch = "wasm32")]
#[link_section = "component-type:wit-bindgen:0.35.0:lay3r:avs@0.3.0:eth-event-world:encoded world"]
#[doc(hidden)]
pub static __WIT_BINDGEN_COMPONENT_TYPE: [u8; 334] = *b"\
\0asm\x0d\0\x01\0\0\x19\x16wit-component-encoding\x04\0\x07\xc8\x01\x01A\x02\x01\
A\x08\x01B\x04\x01p}\x01p\0\x01r\x03\x07address\0\x0alog-topics\x01\x08log-data\0\
\x04\0\x07eth-log\x03\0\x02\x03\0\x1flay3r:avs/eth-event-types@0.3.0\x05\0\x02\x03\
\0\0\x07eth-log\x03\0\x07eth-log\x03\0\x01\x01p}\x01j\x01\x03\x01s\x01@\x01\x03l\
og\x02\0\x04\x04\0\x11process-eth-event\x01\x05\x04\0\x1flay3r:avs/eth-event-wor\
ld@0.3.0\x04\0\x0b\x15\x01\0\x0feth-event-world\x03\0\0\0G\x09producers\x01\x0cp\
rocessed-by\x02\x0dwit-component\x070.220.0\x10wit-bindgen-rust\x060.35.0";
#[inline(never)]
#[doc(hidden)]
pub fn __link_custom_section_describing_imports() {
    wit_bindgen_rt::maybe_link_cabi_realloc();
}
