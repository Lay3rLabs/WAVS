use web_sys::HtmlTextAreaElement;

use crate::prelude::*;

pub struct TextArea {
    pub placeholder: Option<String>,
    pub on_input: Option<Arc<dyn Fn(Option<String>)>>,
    pub initial_value: Option<String>,
    pub mixin: Option<Box<dyn MixinFnOnce<HtmlTextAreaElement>>>,
}

impl TextArea {
    pub fn new() -> Self {
        Self {
            placeholder: None,
            on_input: None,
            initial_value: None,
            mixin: None,
        }
    }

    pub fn with_on_input(mut self, on_input: impl Fn(Option<String>) + 'static) -> Self {
        self.on_input = Some(Arc::new(on_input));
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl ToString) -> Self {
        self.placeholder = Some(placeholder.to_string());
        self
    }

    pub fn with_intial_value(mut self, value: impl ToString) -> Self {
        self.initial_value = Some(value.to_string());
        self
    }

    pub fn with_mixin(mut self, mixin: impl MixinFnOnce<HtmlTextAreaElement> + 'static) -> Self {
        self.mixin = Some(Box::new(mixin));
        self
    }

    pub fn render(self) -> Dom {
        let Self {
            placeholder,
            on_input,
            initial_value,
            mixin,
        } = self;

        html!("textarea" => HtmlTextAreaElement, {
            .attrs!{
                "autocomplete": "off",
                "spellcheck": "false",
                "autocorrect": "off"
            }
            .apply_if(placeholder.is_some(), |dom| {
                dom.attr("placeholder", &placeholder.unwrap_throw())
            })
            .apply_if(initial_value.is_some(), |dom| {
                dom.text(&initial_value.unwrap_throw())
            })

            .apply_if(mixin.is_some(), |dom| {
                mixin.unwrap_throw()(dom)
            })

            .with_node!(elem => {
                .apply_if(on_input.is_some(), move |dom| {
                    let on_input = on_input.unwrap_throw();
                    dom
                        .event(clone!(on_input => move |_:events::Input| {
                            let text = elem.value();
                            let text = if text.is_empty() {
                                None
                            } else {
                                Some(text)
                            };

                            on_input(text);
                        }))
                })
            })
        })
    }
}
