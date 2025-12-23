use crate::prelude::*;
use std::pin::Pin;
use web_sys::HtmlElement;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ButtonSize {
    Sm,
    Lg,
    Xlg,
}

impl ButtonSize {
    pub fn text_size_class(self) -> &'static str {
        match self {
            Self::Sm => FontSize::Sm.class(),
            Self::Lg => FontSize::Lg.class(),
            Self::Xlg => FontSize::Xlg.class(),
        }
    }

    pub fn container_class(self) -> &'static str {
        static DEFAULT_CLASS: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("padding", "0.625rem 1.875rem")
            }
        });

        static SM_CLASS: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("padding", "0.375rem 1.25rem")
            }
        });

        match self {
            Self::Sm => &SM_CLASS,
            _ => &DEFAULT_CLASS,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ButtonColor {
    Primary,
    Red,
    Purple,
}

impl ButtonColor {
    pub fn bg_class(&self, style: ButtonStyle, state: ButtonState) -> &'static str {
        match state {
            ButtonState::Disabled => match style {
                ButtonStyle::Solid => ColorBackground::ButtonDisabled.class(),
                ButtonStyle::Outline => ColorBackground::Initial.class(),
            },
            ButtonState::Selected => match style {
                ButtonStyle::Solid => match self {
                    Self::Primary => ColorBackground::ButtonPrimarySelected.class(),
                    Self::Red => ColorBackground::ButtonRedSelected.class(),
                    Self::Purple => ColorBackground::ButtonPurpleSelected.class(),
                },
                ButtonStyle::Outline => ColorBackground::Initial.class(),
            },
            ButtonState::Hover => match style {
                ButtonStyle::Solid => match self {
                    Self::Primary => ColorBackground::ButtonPrimaryHover.class(),
                    Self::Red => ColorBackground::ButtonRedHover.class(),
                    Self::Purple => ColorBackground::ButtonPurpleHover.class(),
                },
                ButtonStyle::Outline => ColorBackground::Initial.class(),
            },
            ButtonState::Normal => match style {
                ButtonStyle::Solid => match self {
                    Self::Primary => ColorBackground::ButtonPrimary.class(),
                    Self::Red => ColorBackground::ButtonRed.class(),
                    Self::Purple => ColorBackground::ButtonPurple.class(),
                },
                ButtonStyle::Outline => ColorBackground::Initial.class(),
            },
        }
    }

    pub fn border_class(&self, style: ButtonStyle, state: ButtonState) -> &'static str {
        match state {
            ButtonState::Disabled => match style {
                ButtonStyle::Solid => ColorBorder::Initial.class(),
                ButtonStyle::Outline => ColorBorder::ButtonDisabled.class(),
            },
            ButtonState::Selected => match style {
                ButtonStyle::Solid => match self {
                    Self::Primary => ColorBorder::ButtonSolidPrimarySelected.class(),
                    Self::Red => ColorBorder::ButtonSolidRedSelected.class(),
                    Self::Purple => ColorBorder::ButtonSolidPurpleSelected.class(),
                },
                ButtonStyle::Outline => match self {
                    Self::Primary => ColorBorder::ButtonOutlinePrimarySelected.class(),
                    Self::Red => ColorBorder::ButtonOutlineRedSelected.class(),
                    Self::Purple => ColorBorder::ButtonOutlinePurpleSelected.class(),
                },
            },
            ButtonState::Hover => match style {
                ButtonStyle::Solid => ColorBorder::Initial.class(),
                ButtonStyle::Outline => match self {
                    Self::Primary => ColorBorder::ButtonOutlinePrimaryHover.class(),
                    Self::Red => ColorBorder::ButtonOutlineRedHover.class(),
                    Self::Purple => ColorBorder::ButtonOutlinePurpleHover.class(),
                },
            },
            ButtonState::Normal => match style {
                ButtonStyle::Solid => ColorBorder::Initial.class(),
                ButtonStyle::Outline => match self {
                    Self::Primary => ColorBorder::ButtonOutlinePrimary.class(),
                    Self::Red => ColorBorder::ButtonOutlineRed.class(),
                    Self::Purple => ColorBorder::ButtonOutlinePurple.class(),
                },
            },
        }
    }

    pub fn color_class(&self, style: ButtonStyle, state: ButtonState) -> &'static str {
        match state {
            ButtonState::Disabled => match style {
                ButtonStyle::Solid => match self {
                    Self::Primary => ColorText::ButtonPrimary.class(),
                    Self::Red => ColorText::ButtonPrimary.class(),
                    Self::Purple => ColorText::ButtonPrimary.class(),
                },
                ButtonStyle::Outline => ColorBackground::Initial.class(),
            },
            ButtonState::Selected => match style {
                ButtonStyle::Solid => match self {
                    Self::Primary => ColorText::ButtonPrimarySelected.class(),
                    Self::Red => ColorText::ButtonPrimarySelected.class(),
                    Self::Purple => ColorText::ButtonPrimarySelected.class(),
                },
                ButtonStyle::Outline => match self {
                    Self::Primary => ColorText::ButtonOutlinePrimarySelected.class(),
                    Self::Red => ColorText::ButtonOutlineRedSelected.class(),
                    Self::Purple => ColorText::ButtonOutlinePurpleSelected.class(),
                },
            },
            ButtonState::Hover => match style {
                ButtonStyle::Solid => match self {
                    Self::Primary => ColorText::ButtonPrimaryHover.class(),
                    Self::Red => ColorText::ButtonPrimaryHover.class(),
                    Self::Purple => ColorText::ButtonPrimaryHover.class(),
                },
                ButtonStyle::Outline => match self {
                    Self::Primary => ColorText::ButtonOutlinePrimaryHover.class(),
                    Self::Red => ColorText::ButtonOutlineRedHover.class(),
                    Self::Purple => ColorText::ButtonOutlinePurpleHover.class(),
                },
            },
            ButtonState::Normal => match style {
                ButtonStyle::Solid => match self {
                    Self::Primary => ColorText::ButtonPrimary.class(),
                    Self::Red => ColorText::ButtonPrimary.class(),
                    Self::Purple => ColorText::ButtonPrimary.class(),
                },
                ButtonStyle::Outline => match self {
                    Self::Primary => ColorText::ButtonOutlinePrimary.class(),
                    Self::Red => ColorText::ButtonOutlineRed.class(),
                    Self::Purple => ColorText::ButtonOutlinePurple.class(),
                },
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ButtonStyle {
    Solid,
    Outline,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ButtonState {
    Normal,
    Hover,
    Selected,
    Disabled,
}

pub struct Button {
    size: ButtonSize,
    style: ButtonStyle,
    color: ButtonColor,
    text: String,
    disabled_signal: Option<Pin<Box<dyn Signal<Item = bool>>>>,
    selected_signal: Option<Pin<Box<dyn Signal<Item = bool>>>>,
    on_click: Option<Box<dyn FnMut()>>,
    link: Option<String>,
    content_before: Option<Dom>,
    content_after: Option<Dom>,
    mixin: Option<Box<dyn MixinFnOnce<HtmlElement>>>,
}

impl Button {
    pub fn new() -> Self {
        Self {
            size: ButtonSize::Lg,
            style: ButtonStyle::Solid,
            color: ButtonColor::Primary,
            text: "".to_string(),
            content_before: None,
            content_after: None,
            disabled_signal: None,
            selected_signal: None,
            on_click: None,
            mixin: None,
            link: None,
        }
    }

    pub fn with_text(mut self, text: impl ToString) -> Self {
        self.text = text.to_string();
        self
    }

    pub fn with_content_before(mut self, content: Dom) -> Self {
        self.content_before = Some(content);
        self
    }

    pub fn with_content_after(mut self, content: Dom) -> Self {
        self.content_after = Some(content);
        self
    }

    pub fn with_style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_link(mut self, link: impl ToString) -> Self {
        self.link = Some(link.to_string());
        self
    }

    pub fn with_size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    pub fn with_color(mut self, color: ButtonColor) -> Self {
        self.color = color;
        self
    }

    pub fn with_disabled_signal(
        mut self,
        disabled_signal: impl Signal<Item = bool> + 'static,
    ) -> Self {
        self.disabled_signal = Some(Box::pin(disabled_signal));
        self
    }

    pub fn with_selected_signal(
        mut self,
        selected_signal: impl Signal<Item = bool> + 'static,
    ) -> Self {
        self.selected_signal = Some(Box::pin(selected_signal));
        self
    }

    pub fn with_on_click(mut self, on_click: impl FnMut() + 'static) -> Self {
        self.on_click = Some(Box::new(on_click));
        self
    }

    pub fn with_mixin(mut self, mixin: impl MixinFnOnce<HtmlElement> + 'static) -> Self {
        self.mixin = Some(Box::new(mixin));
        self
    }

    pub fn render(self) -> Dom {
        static CLASS: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "inline-flex")
                .style("justify-content", "center")
                .style("align-items", "center")
                .style("gap", "0.625rem")
                .style("border-radius", "0.25rem")
                .style("width", "fit-content")
                .style(["user-select", "-webkit-user-select"], "none")
            }
        });

        static BORDER_CLASS: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("border-width", "1px")
                .style("border-style", "solid")
            }
        });

        let Self {
            size,
            color,
            text,
            disabled_signal,
            selected_signal,
            content_before,
            content_after,
            mut on_click,
            style,
            mixin,
            link,
        } = self;

        let hovering = Mutable::new(false);
        let disabled = Mutable::new(false);
        let selected = Mutable::new(false);

        // Combine all state into a single ButtonState signal and broadcast it
        let state_signal = map_ref! {
            let disabled = disabled.signal(),
            let selected = selected.signal(),
            let hovering = hovering.signal() => {
                if *disabled {
                    ButtonState::Disabled
                } else if *selected {
                    ButtonState::Selected
                } else if *hovering {
                    ButtonState::Hover
                } else {
                    ButtonState::Normal
                }
            }
        }
        .broadcast();

        let cursor_signal = map_ref! {
            let disabled = disabled.signal(),
            let hovering = hovering.signal() => {
                if *disabled {
                    "not-allowed"
                } else if *hovering {
                    "pointer"
                } else {
                    "auto"
                }
            }
        };

        let ret = html!("div", {
            .apply_if(disabled_signal.is_some(), clone!(disabled => move |dom| {
                dom
                    .future(disabled_signal.unwrap_throw().for_each(clone!(disabled => move |is_disabled| {
                        clone!(disabled => async move {
                            disabled.set_neq(is_disabled);
                        })
                    })))
            }))
            .apply_if(selected_signal.is_some(), clone!(selected => move |dom| {
                dom
                    .future(selected_signal.unwrap_throw().for_each(clone!(selected => move |is_selected| {
                        clone!(selected => async move {
                            selected.set_neq(is_selected);
                        })
                    })))
            }))
            .class([&*CLASS, size.container_class(), size.text_size_class()])
            .apply(set_on_hover(&hovering))
            .style_signal("cursor", cursor_signal)
            .class_signal(&*BORDER_CLASS, state_signal.signal().map(clone!(style => move |s| {
                // Apply border for outline buttons always, or for solid buttons when selected
                style == ButtonStyle::Outline || s == ButtonState::Selected
            })))
            .class_signal([color.bg_class(style, ButtonState::Normal), color.border_class(style, ButtonState::Normal)], state_signal.signal().map(|s| s == ButtonState::Normal))
            .class_signal([color.bg_class(style, ButtonState::Hover), color.border_class(style, ButtonState::Hover)], state_signal.signal().map(|s| s == ButtonState::Hover))
            .class_signal([color.bg_class(style, ButtonState::Selected), color.border_class(style, ButtonState::Selected)], state_signal.signal().map(|s| s == ButtonState::Selected))
            .class_signal([color.bg_class(style, ButtonState::Disabled), color.border_class(style, ButtonState::Disabled)], state_signal.signal().map(|s| s == ButtonState::Disabled))
            .apply(handle_on_click(clone!(disabled => move || {
                if !disabled.get() {
                    if let Some(on_click) = &mut on_click {
                        on_click();
                    }
                }
            })))
            .apply_if(mixin.is_some(), |dom| {
                mixin.unwrap_throw()(dom)
            })
            .apply_if(content_before.is_some(), |dom| {
                dom.child(content_before.unwrap_throw())
            })
            .child(html!("div", {
                    .class_signal(color.color_class(style, ButtonState::Normal), state_signal.signal().map(|s| s == ButtonState::Normal))
                    .class_signal(color.color_class(style, ButtonState::Hover), state_signal.signal().map(|s| s == ButtonState::Hover))
                    .class_signal(color.color_class(style, ButtonState::Selected), state_signal.signal().map(|s| s == ButtonState::Selected))
                    .class_signal(color.color_class(style, ButtonState::Disabled), state_signal.signal().map(|s| s == ButtonState::Disabled))
                    .text(&text)
            }))
            .apply_if(content_after.is_some(), |dom| {
                dom.child(content_after.unwrap_throw())
            })
        });

        match link {
            Some(link) => {
                link!(link, {
                    .child(ret)
                })
            }
            None => ret,
        }
    }
}
