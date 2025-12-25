use crate::prelude::*;
use web_sys::HtmlInputElement;

pub struct TextInput {
    pub kind: TextInputKind,
    pub placeholder: Option<String>,
    pub on_input: Option<Arc<dyn Fn(Option<String>)>>,
    pub initial_value: Option<String>,
    pub mixin: Option<Box<dyn MixinFnOnce<HtmlInputElement>>>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum TextInputKind {
    Email,
    Password,
    Text,
    Number,
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            kind: TextInputKind::Text,
            placeholder: None,
            on_input: None,
            initial_value: None,
            mixin: None,
        }
    }

    pub fn with_kind(mut self, kind: TextInputKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_on_input(mut self, on_input: impl Fn(Option<String>) + 'static) -> Self {
        self.on_input = Some(Arc::new(on_input));
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl ToString) -> Self {
        let placeholder = placeholder.to_string();
        self.placeholder = if placeholder.is_empty() {
            None
        } else {
            Some(placeholder)
        };
        self
    }

    pub fn with_intial_value(mut self, value: impl ToString) -> Self {
        let initial_value = value.to_string();
        self.initial_value = if initial_value.is_empty() {
            None
        } else {
            Some(initial_value)
        };
        self
    }

    pub fn with_mixin(mut self, mixin: impl MixinFnOnce<HtmlInputElement> + 'static) -> Self {
        self.mixin = Some(Box::new(mixin));
        self
    }

    pub fn render(self) -> Dom {
        static CLASS: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("padding", "0.625rem 1.875rem")
                .style("border-radius", "0.25rem")
                .style("border-width", "1px")
                .style("border-style", "solid")
            }
        });

        let show_password = Mutable::new(false);

        let Self {
            kind,
            placeholder,
            on_input,
            initial_value,
            mixin,
        } = self;

        html!("div", {
            .child(html!("input" => web_sys::HtmlInputElement, {
                .class(&*CLASS)
                .attrs!{
                    "autocomplete": "off",
                    "spellcheck": "false",
                    "autocorrect": "off"
                }
                .attr_signal("type", show_password.signal().map(move |show_password| {
                    match kind {
                        TextInputKind::Email => "email",
                        TextInputKind::Password => if show_password { "text" } else {"password"},
                        TextInputKind::Text => "text",
                        TextInputKind::Number => "number",
                    }
                }))
                .apply_if(placeholder.is_some(), |dom| {
                    dom.attr("placeholder", &placeholder.unwrap_throw())
                })
                .apply_if(initial_value.is_some(), |dom| {
                    dom.attr("value", &initial_value.unwrap_throw())
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
            }))

            .apply_if(self.kind == TextInputKind::Password, |dom| {
                dom.child(html!("div", {
                    .style("margin-top", "0.625rem")
                    .style("cursor", "pointer")
                    .class(FontSize::Md.class())
                    .text_signal(show_password.signal().map(|show_password| {
                        if show_password {
                            "Hide password".to_string()
                        } else {
                            "Show password".to_string()
                        }
                    }))
                    .event(clone!(show_password => move |_:events::Click| {
                        show_password.replace_with(|x| !*x);
                    }))
                }))
            })
        })
    }
}
