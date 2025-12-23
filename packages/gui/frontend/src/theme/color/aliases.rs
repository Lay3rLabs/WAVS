use crate::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ColorBackground {
    Initial,
    NavBar,
    MainContent,
    ButtonPrimary,
    ButtonPrimaryHover,
    ButtonPrimarySelected,
    ButtonDisabled,
    ButtonRed,
    ButtonRedHover,
    ButtonRedSelected,
    ButtonPurple,
    ButtonPurpleHover,
    ButtonPurpleSelected,
}

impl ColorBackground {
    pub fn value(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::NavBar => ColorRaw::CharcoalDarkest.value(),
            Self::MainContent => ColorRaw::CharcoalDarkest.value(),
            Self::ButtonPrimary => ColorRaw::CharcoalMedium.value(),
            Self::ButtonPrimaryHover => ColorRaw::CharcoalDark.value(),
            Self::ButtonPrimarySelected => ColorRaw::CharcoalLight.value(),
            Self::ButtonDisabled => ColorRaw::CharcoalDark.value(),
            Self::ButtonRed => "#991b1b",
            Self::ButtonRedHover => "#b91c1c",
            Self::ButtonRedSelected => "#dc2626",
            Self::ButtonPurple => "#7e22ce",
            Self::ButtonPurpleHover => "#9333ea",
            Self::ButtonPurpleSelected => "#a855f7",
        }
    }

    pub fn class(self) -> &'static str {
        static INITIAL: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("background-color", ColorBackground::Initial.value())
            }
        });

        static NAV_BAR: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("background-color", ColorBackground::NavBar.value())
            }
        });

        static MAIN_CONTENT: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("background-color", ColorBackground::MainContent.value())
            }
        });

        static BUTTON_PRIMARY: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("background-color", ColorBackground::ButtonPrimary.value())
            }
        });

        static BUTTON_PRIMARY_HOVER: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("background-color", ColorBackground::ButtonPrimaryHover.value())
            }
        });

        static BUTTON_DISABLED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("background-color", ColorBackground::ButtonDisabled.value())
            }
        });

        static BUTTON_RED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("background-color", ColorBackground::ButtonRed.value())
            }
        });

        static BUTTON_RED_HOVER: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("background-color", ColorBackground::ButtonRedHover.value())
            }
        });

        static BUTTON_PRIMARY_SELECTED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("background-color", ColorBackground::ButtonPrimarySelected.value())
            }
        });

        static BUTTON_RED_SELECTED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("background-color", ColorBackground::ButtonRedSelected.value())
            }
        });

        static BUTTON_PURPLE: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("background-color", ColorBackground::ButtonPurple.value())
            }
        });

        static BUTTON_PURPLE_HOVER: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("background-color", ColorBackground::ButtonPurpleHover.value())
            }
        });

        static BUTTON_PURPLE_SELECTED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("background-color", ColorBackground::ButtonPurpleSelected.value())
            }
        });

        match self {
            Self::Initial => &INITIAL,
            Self::NavBar => &NAV_BAR,
            Self::MainContent => &MAIN_CONTENT,
            Self::ButtonPrimary => &BUTTON_PRIMARY,
            Self::ButtonPrimaryHover => &BUTTON_PRIMARY_HOVER,
            Self::ButtonPrimarySelected => &BUTTON_PRIMARY_SELECTED,
            Self::ButtonDisabled => &BUTTON_DISABLED,
            Self::ButtonRed => &BUTTON_RED,
            Self::ButtonRedHover => &BUTTON_RED_HOVER,
            Self::ButtonRedSelected => &BUTTON_RED_SELECTED,
            Self::ButtonPurple => &BUTTON_PURPLE,
            Self::ButtonPurpleHover => &BUTTON_PURPLE_HOVER,
            Self::ButtonPurpleSelected => &BUTTON_PURPLE_SELECTED,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ColorBorder {
    Initial,
    ButtonSolidPrimarySelected,
    ButtonSolidRedSelected,
    ButtonSolidPurpleSelected,
    ButtonOutlinePrimary,
    ButtonOutlinePrimaryHover,
    ButtonOutlinePrimarySelected,
    ButtonOutlineRed,
    ButtonOutlineRedHover,
    ButtonOutlineRedSelected,
    ButtonOutlinePurple,
    ButtonOutlinePurpleHover,
    ButtonOutlinePurpleSelected,
    ButtonDisabled,
}

impl ColorBorder {
    pub fn value(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::ButtonSolidPrimarySelected => ColorRaw::BeigeWarm.value(),
            Self::ButtonSolidRedSelected => "#dc2626",
            Self::ButtonSolidPurpleSelected => "#a855f7",
            Self::ButtonOutlinePrimary => ColorRaw::TanWarm.value(),
            Self::ButtonOutlinePrimaryHover => ColorRaw::BeigeWarm.value(),
            Self::ButtonOutlinePrimarySelected => ColorRaw::BeigeLight.value(),
            Self::ButtonOutlineRed => ColorRaw::TanMuted.value(),
            Self::ButtonOutlineRedHover => ColorRaw::TanWarm.value(),
            Self::ButtonOutlineRedSelected => ColorRaw::BeigeWarm.value(),
            Self::ButtonOutlinePurple => "#7e22ce",
            Self::ButtonOutlinePurpleHover => "#9333ea",
            Self::ButtonOutlinePurpleSelected => "#a855f7",
            Self::ButtonDisabled => ColorRaw::CharcoalMedium.value(),
        }
    }

    pub fn class(self) -> &'static str {
        static INITIAL: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::Initial.value())
            }
        });

        static BUTTON_OUTLINE_PRIMARY: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::ButtonOutlinePrimary.value())
            }
        });

        static BUTTON_OUTLINE_PRIMARY_HOVER: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::ButtonOutlinePrimaryHover.value())
            }
        });

        static BUTTON_OUTLINE_RED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::ButtonOutlineRed.value())
            }
        });

        static BUTTON_OUTLINE_RED_HOVER: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::ButtonOutlineRedHover.value())
            }
        });

        static BUTTON_DISABLED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::ButtonDisabled.value())
            }
        });

        static BUTTON_OUTLINE_PRIMARY_SELECTED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::ButtonOutlinePrimarySelected.value())
            }
        });

        static BUTTON_OUTLINE_RED_SELECTED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::ButtonOutlineRedSelected.value())
            }
        });

        static BUTTON_SOLID_PRIMARY_SELECTED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::ButtonSolidPrimarySelected.value())
            }
        });

        static BUTTON_SOLID_RED_SELECTED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::ButtonSolidRedSelected.value())
            }
        });

        static BUTTON_SOLID_PURPLE_SELECTED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::ButtonSolidPurpleSelected.value())
            }
        });

        static BUTTON_OUTLINE_PURPLE: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::ButtonOutlinePurple.value())
            }
        });

        static BUTTON_OUTLINE_PURPLE_HOVER: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::ButtonOutlinePurpleHover.value())
            }
        });

        static BUTTON_OUTLINE_PURPLE_SELECTED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("border-color", ColorBorder::ButtonOutlinePurpleSelected.value())
            }
        });

        match self {
            Self::Initial => &INITIAL,
            Self::ButtonSolidPrimarySelected => &BUTTON_SOLID_PRIMARY_SELECTED,
            Self::ButtonSolidRedSelected => &BUTTON_SOLID_RED_SELECTED,
            Self::ButtonSolidPurpleSelected => &BUTTON_SOLID_PURPLE_SELECTED,
            Self::ButtonOutlinePrimary => &BUTTON_OUTLINE_PRIMARY,
            Self::ButtonOutlinePrimaryHover => &BUTTON_OUTLINE_PRIMARY_HOVER,
            Self::ButtonOutlinePrimarySelected => &BUTTON_OUTLINE_PRIMARY_SELECTED,
            Self::ButtonOutlineRed => &BUTTON_OUTLINE_RED,
            Self::ButtonOutlineRedHover => &BUTTON_OUTLINE_RED_HOVER,
            Self::ButtonOutlineRedSelected => &BUTTON_OUTLINE_RED_SELECTED,
            Self::ButtonOutlinePurple => &BUTTON_OUTLINE_PURPLE,
            Self::ButtonOutlinePurpleHover => &BUTTON_OUTLINE_PURPLE_HOVER,
            Self::ButtonOutlinePurpleSelected => &BUTTON_OUTLINE_PURPLE_SELECTED,
            Self::ButtonDisabled => &BUTTON_DISABLED,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ColorText {
    NavBar,
    MainContent,
    ButtonPrimary,
    ButtonPrimaryHover,
    ButtonPrimarySelected,
    ButtonOutlinePrimary,
    ButtonOutlinePrimaryHover,
    ButtonOutlinePrimarySelected,
    ButtonOutlineRed,
    ButtonOutlineRedHover,
    ButtonOutlineRedSelected,
    ButtonOutlinePurple,
    ButtonOutlinePurpleHover,
    ButtonOutlinePurpleSelected,
    RedAlert,
}

impl ColorText {
    pub fn value(self) -> &'static str {
        match self {
            Self::NavBar => ColorRaw::BeigeWarm.value(),
            Self::MainContent => ColorRaw::BeigeWarm.value(),
            Self::ButtonPrimary => ColorRaw::BeigeWarm.value(),
            Self::ButtonPrimaryHover => ColorRaw::BeigeLight.value(),
            Self::ButtonPrimarySelected => ColorRaw::Whiteish.value(),
            Self::ButtonOutlinePrimary => ColorRaw::TanWarm.value(),
            Self::ButtonOutlinePrimaryHover => ColorRaw::BeigeWarm.value(),
            Self::ButtonOutlinePrimarySelected => ColorRaw::BeigeLight.value(),
            Self::ButtonOutlineRed => ColorRaw::TanMuted.value(),
            Self::ButtonOutlineRedHover => ColorRaw::TanWarm.value(),
            Self::ButtonOutlineRedSelected => ColorRaw::BeigeWarm.value(),
            Self::ButtonOutlinePurple => "#7e22ce",
            Self::ButtonOutlinePurpleHover => "#9333ea",
            Self::ButtonOutlinePurpleSelected => "#a855f7",
            Self::RedAlert => "#ef4444",
        }
    }

    pub fn class(self) -> &'static str {
        static NAV_BAR: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::NavBar.value())
            }
        });

        static MAIN_CONTENT: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::MainContent.value())
            }
        });

        static BUTTON_PRIMARY: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::ButtonPrimary.value())
            }
        });

        static BUTTON_OUTLINE_PRIMARY: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::ButtonOutlinePrimary.value())
            }
        });

        static BUTTON_OUTLINE_PRIMARY_HOVER: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::ButtonOutlinePrimaryHover.value())
            }
        });

        static BUTTON_OUTLINE_RED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::ButtonOutlineRed.value())
            }
        });

        static BUTTON_OUTLINE_RED_HOVER: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::ButtonOutlineRedHover.value())
            }
        });

        static BUTTON_PRIMARY_HOVER: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::ButtonPrimaryHover.value())
            }
        });

        static BUTTON_PRIMARY_SELECTED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::ButtonPrimarySelected.value())
            }
        });

        static BUTTON_OUTLINE_PRIMARY_SELECTED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::ButtonOutlinePrimarySelected.value())
            }
        });

        static BUTTON_OUTLINE_RED_SELECTED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::ButtonOutlineRedSelected.value())
            }
        });

        static BUTTON_OUTLINE_PURPLE: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::ButtonOutlinePurple.value())
            }
        });

        static BUTTON_OUTLINE_PURPLE_HOVER: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::ButtonOutlinePurpleHover.value())
            }
        });

        static BUTTON_OUTLINE_PURPLE_SELECTED: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::ButtonOutlinePurpleSelected.value())
            }
        });

        static RED_ALERT: LazyLock<String> = LazyLock::new(|| {
            class! {
              .style("color", ColorText::RedAlert.value())
            }
        });

        match self {
            Self::NavBar => &NAV_BAR,
            Self::MainContent => &MAIN_CONTENT,
            Self::ButtonPrimary => &BUTTON_PRIMARY,
            Self::ButtonPrimaryHover => &BUTTON_PRIMARY_HOVER,
            Self::ButtonPrimarySelected => &BUTTON_PRIMARY_SELECTED,
            Self::ButtonOutlinePrimary => &BUTTON_OUTLINE_PRIMARY,
            Self::ButtonOutlinePrimaryHover => &BUTTON_OUTLINE_PRIMARY_HOVER,
            Self::ButtonOutlinePrimarySelected => &BUTTON_OUTLINE_PRIMARY_SELECTED,
            Self::ButtonOutlineRed => &BUTTON_OUTLINE_RED,
            Self::ButtonOutlineRedHover => &BUTTON_OUTLINE_RED_HOVER,
            Self::ButtonOutlineRedSelected => &BUTTON_OUTLINE_RED_SELECTED,
            Self::ButtonOutlinePurple => &BUTTON_OUTLINE_PURPLE,
            Self::ButtonOutlinePurpleHover => &BUTTON_OUTLINE_PURPLE_HOVER,
            Self::ButtonOutlinePurpleSelected => &BUTTON_OUTLINE_PURPLE_SELECTED,
            Self::RedAlert => &RED_ALERT,
        }
    }
}
