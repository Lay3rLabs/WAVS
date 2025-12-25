#![allow(unused_imports, dead_code)]
pub use super::{
    atoms::*,
    route::*,
    state::*,
    theme::{color::*, misc::*, typography::*, z_index::*},
    util::{mixins::*, signal::*},
};
pub use crate::config::CONFIG;
pub use awsm_web::prelude::UnwrapExt;
pub use wasm_bindgen::prelude::*;

pub use anyhow::{anyhow, bail, Context as AnyhowContext, Result};
use dominator::DomBuilder;
pub use dominator::{
    apply_methods, attrs, class, clone, events, fragment, html, link, pseudo, styles, svg,
    with_node, Dom, Fragment,
};
pub use futures_signals::{
    map_ref,
    signal::{Mutable, Signal, SignalExt},
    signal_vec::{MutableVec, SignalVec, SignalVecExt},
};
pub use serde::{Deserialize, Serialize};
pub use std::sync::{Arc, LazyLock, Mutex, RwLock};

// mixin aliases and helper traits
pub type MixinStub<T> = fn(DomBuilder<T>) -> DomBuilder<T>;

pub trait MixinFnOnce<T>: FnOnce(DomBuilder<T>) -> DomBuilder<T> {}
impl<T, F> MixinFnOnce<T> for F where F: FnOnce(DomBuilder<T>) -> DomBuilder<T> {}

pub trait MixinFn<T>: Fn(DomBuilder<T>) -> DomBuilder<T> {}
impl<T, F> MixinFn<T> for F where F: Fn(DomBuilder<T>) -> DomBuilder<T> {}
