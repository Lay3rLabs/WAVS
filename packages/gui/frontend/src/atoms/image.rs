use std::sync::{Arc, Mutex};

use awsm_web::prelude::UnwrapExt;
use dominator::{clone, fragment, html, DomBuilder, Fragment};
use dominator_helpers::futures::AsyncLoader;
use futures::future;
use futures_signals::signal::{Mutable, SignalExt};
use web_sys::{HtmlElement, HtmlImageElement};

use crate::prelude::MixinFnOnce;

pub struct Image {
    // relative to the media root
    pub src: String,
    // gotta wrap the mixin in an Arc<Mutex> so we can move it into the signal mapper
    // but we will only be taking it out of the mutex once
    mixin: Option<Box<dyn MixinFnOnce<HtmlImageElement> + 'static>>,
}

impl Image {
    pub fn new(src: impl Into<String>) -> Self {
        Self {
            src: src.into(),
            mixin: None,
        }
    }
    pub fn with_mixin(mut self, mixin: impl MixinFnOnce<HtmlImageElement> + 'static) -> Self {
        self.mixin = Some(Box::new(mixin));
        self
    }

    pub fn render(self) -> impl Fragment {
        render_app_img(self.src, move |dom| {
            if let Some(mixin) = self.mixin {
                mixin(dom)
            } else {
                dom
            }
        })
    }
}

pub fn render_app_img<F>(path: String, mixin: F) -> impl Fragment
where
    F: FnOnce(DomBuilder<HtmlImageElement>) -> DomBuilder<HtmlImageElement> + 'static,
{
    let url = Mutable::new(None);

    // gotta wrap the mixin in an Arc<Mutex> so we can move it into the signal mapper
    // but we will only be taking it out of the mutex once
    let mixin = Arc::new(Mutex::new(Some(mixin)));

    fragment!(move {
        // in its own div so we don't need to worry about how the image is affected by the container
        // using an element so the future will be dropped when the element is removed
        .child(html!("div", {
            .future(clone!(url, path => async move {
                url.set(Some(crate::config::CONFIG.app_image_url(&path).await.unwrap_ext()));
            }))
        }))
        .child_signal(url.signal_cloned().map(clone!(mixin => move |url| {
            url.map(|url| {
                html!("img" => HtmlImageElement, {
                    .apply(|dom| {
                        let mixin = mixin.lock().unwrap_ext().take().unwrap_ext();
                        mixin(dom)
                    })
                    .attr("src", &url)
                })
            })
        })))
    })
}
