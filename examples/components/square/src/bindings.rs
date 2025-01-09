pub type Contract = lay3r::avs::wavs_types::Contract;
#[doc(hidden)]
#[allow(non_snake_case)]
pub unsafe fn _export_run_cabi<T: Guest>(
    arg0: i32,
    arg1: *mut u8,
    arg2: usize,
    arg3: i32,
    arg4: *mut u8,
    arg5: usize,
    arg6: *mut u8,
    arg7: usize,
) -> *mut u8 {
    #[cfg(target_arch = "wasm32")]
    _rt::run_ctors_once();
    use lay3r::avs::wavs_types::Address as V2;
    let v2 = match arg0 {
        0 => {
            let e2 = {
                let len0 = arg2;
                _rt::Vec::from_raw_parts(arg1.cast(), len0, len0)
            };
            V2::Eth(e2)
        }
        n => {
            debug_assert_eq!(n, 1, "invalid enum discriminant");
            let e2 = {
                let len1 = arg2;
                let bytes1 = _rt::Vec::from_raw_parts(arg1.cast(), len1, len1);
                (_rt::string_lift(bytes1), arg3 as u32)
            };
            V2::Cosmos(e2)
        }
    };
    let len3 = arg5;
    let bytes3 = _rt::Vec::from_raw_parts(arg4.cast(), len3, len3);
    let len4 = arg7;
    let result5 = T::run(
        lay3r::avs::wavs_types::Contract {
            address: v2,
            chain_id: _rt::string_lift(bytes3),
        },
        _rt::Vec::from_raw_parts(arg6.cast(), len4, len4),
    );
    let ptr6 = _RET_AREA.0.as_mut_ptr().cast::<u8>();
    match result5 {
        Ok(e) => {
            *ptr6.add(0).cast::<u8>() = (0i32) as u8;
            let vec7 = (e).into_boxed_slice();
            let ptr7 = vec7.as_ptr().cast::<u8>();
            let len7 = vec7.len();
            ::core::mem::forget(vec7);
            *ptr6.add(8).cast::<usize>() = len7;
            *ptr6.add(4).cast::<*mut u8>() = ptr7.cast_mut();
        }
        Err(e) => {
            *ptr6.add(0).cast::<u8>() = (1i32) as u8;
            let vec8 = (e.into_bytes()).into_boxed_slice();
            let ptr8 = vec8.as_ptr().cast::<u8>();
            let len8 = vec8.len();
            ::core::mem::forget(vec8);
            *ptr6.add(8).cast::<usize>() = len8;
            *ptr6.add(4).cast::<*mut u8>() = ptr8.cast_mut();
        }
    };
    ptr6
}
#[doc(hidden)]
#[allow(non_snake_case)]
pub unsafe fn __post_return_run<T: Guest>(arg0: *mut u8) {
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
    /// event-data is emitted from the chain via a specific event type
    /// which WAVS knows how to extract
    fn run(contract: Contract, event_data: _rt::Vec<u8>) -> Result<_rt::Vec<u8>, _rt::String>;
}
#[doc(hidden)]
macro_rules! __export_world_wavs_chain_event_world_cabi {
    ($ty:ident with_types_in $($path_to_types:tt)*) => {
        const _ : () = { #[export_name = "run"] unsafe extern "C" fn export_run(arg0 :
        i32, arg1 : * mut u8, arg2 : usize, arg3 : i32, arg4 : * mut u8, arg5 : usize,
        arg6 : * mut u8, arg7 : usize,) -> * mut u8 { $($path_to_types)*::
        _export_run_cabi::<$ty > (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7) }
        #[export_name = "cabi_post_run"] unsafe extern "C" fn _post_return_run(arg0 : *
        mut u8,) { $($path_to_types)*:: __post_return_run::<$ty > (arg0) } };
    };
}
#[doc(hidden)]
pub(crate) use __export_world_wavs_chain_event_world_cabi;
#[repr(align(4))]
struct _RetArea([::core::mem::MaybeUninit<u8>; 12]);
static mut _RET_AREA: _RetArea = _RetArea([::core::mem::MaybeUninit::uninit(); 12]);
#[allow(dead_code)]
pub mod lay3r {
    #[allow(dead_code)]
    pub mod avs {
        #[allow(dead_code, clippy::all)]
        pub mod wavs_types {
            #[used]
            #[doc(hidden)]
            static __FORCE_SECTION_REF: fn() =
                super::super::super::__link_custom_section_describing_imports;
            use super::super::super::_rt;
            #[derive(Clone)]
            pub enum Address {
                Eth(_rt::Vec<u8>),
                /// the bech32-encoded address, and length of the prefix
                Cosmos((_rt::String, u32)),
            }
            impl ::core::fmt::Debug for Address {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    match self {
                        Address::Eth(e) => f.debug_tuple("Address::Eth").field(e).finish(),
                        Address::Cosmos(e) => f.debug_tuple("Address::Cosmos").field(e).finish(),
                    }
                }
            }
            #[derive(Clone)]
            pub struct Contract {
                pub address: Address,
                pub chain_id: _rt::String,
            }
            impl ::core::fmt::Debug for Contract {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct("Contract")
                        .field("address", &self.address)
                        .field("chain-id", &self.chain_id)
                        .finish()
                }
            }
            /// An alloy log can be recreated with the info here
            #[derive(Clone)]
            pub struct EthLog {
                /// the raw log topics that can be decoded into an event
                pub topics: _rt::Vec<_rt::Vec<u8>>,
                /// the raw log data that can be decoded into an event
                pub data: _rt::Vec<u8>,
            }
            impl ::core::fmt::Debug for EthLog {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct("EthLog")
                        .field("topics", &self.topics)
                        .field("data", &self.data)
                        .finish()
                }
            }
        }
    }
}
mod _rt {
    pub use alloc_crate::string::String;
    pub use alloc_crate::vec::Vec;
    #[cfg(target_arch = "wasm32")]
    pub fn run_ctors_once() {
        wit_bindgen_rt::run_ctors_once();
    }
    pub unsafe fn string_lift(bytes: Vec<u8>) -> String {
        if cfg!(debug_assertions) {
            String::from_utf8(bytes).unwrap()
        } else {
            String::from_utf8_unchecked(bytes)
        }
    }
    pub unsafe fn cabi_dealloc(ptr: *mut u8, size: usize, align: usize) {
        if size == 0 {
            return;
        }
        let layout = alloc::Layout::from_size_align_unchecked(size, align);
        alloc::dealloc(ptr, layout);
    }
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
macro_rules! __export_wavs_chain_event_world_impl {
    ($ty:ident) => {
        self::export!($ty with_types_in self);
    };
    ($ty:ident with_types_in $($path_to_types_root:tt)*) => {
        $($path_to_types_root)*:: __export_world_wavs_chain_event_world_cabi!($ty
        with_types_in $($path_to_types_root)*);
    };
}
#[doc(inline)]
pub(crate) use __export_wavs_chain_event_world_impl as export;
#[cfg(target_arch = "wasm32")]
#[link_section = "component-type:wit-bindgen:0.35.0:lay3r:avs@0.3.0:wavs-chain-event-world:encoded world"]
#[doc(hidden)]
pub static __WIT_BINDGEN_COMPONENT_TYPE: [u8; 405] = *b"\
\0asm\x0d\0\x01\0\0\x19\x16wit-component-encoding\x04\0\x07\x88\x02\x01A\x02\x01\
A\x08\x01B\x09\x01p}\x01o\x02sy\x01q\x02\x03eth\x01\0\0\x06cosmos\x01\x01\0\x04\0\
\x07address\x03\0\x02\x01r\x02\x07address\x03\x08chain-ids\x04\0\x08contract\x03\
\0\x04\x01p\0\x01r\x02\x06topics\x06\x04data\0\x04\0\x07eth-log\x03\0\x07\x03\0\x1a\
lay3r:avs/wavs-types@0.3.0\x05\0\x02\x03\0\0\x08contract\x03\0\x08contract\x03\0\
\x01\x01p}\x01j\x01\x03\x01s\x01@\x02\x08contract\x02\x0aevent-data\x03\0\x04\x04\
\0\x03run\x01\x05\x04\0&lay3r:avs/wavs-chain-event-world@0.3.0\x04\0\x0b\x1c\x01\
\0\x16wavs-chain-event-world\x03\0\0\0G\x09producers\x01\x0cprocessed-by\x02\x0d\
wit-component\x070.220.0\x10wit-bindgen-rust\x060.35.0";
#[inline(never)]
#[doc(hidden)]
pub fn __link_custom_section_describing_imports() {
    wit_bindgen_rt::maybe_link_cabi_realloc();
}
