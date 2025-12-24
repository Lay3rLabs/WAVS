#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ColorRaw {
    // Dark backgrounds - progressively lighter
    CharcoalDarkest,
    CharcoalDark,
    CharcoalMedium,
    CharcoalLight,

    // Warm text colors - progressively lighter
    TanMuted,
    TanWarm,
    BeigeWarm,
    BeigeLight,
    CreamWarm,
    CreamLight,

    // Neutral extremes
    NearBlack,
    Whiteish,

    // Transparent overlays (for effects/gradients)
    // Note: These will be used with CSS rgba() or similar
    WarmOverlay30, // For subtle warm tints
    DarkOverlay70, // For backdrop effects
    DarkOverlay40, // For gradient stops
    Black15,       // For shadows

    // More stuff
    Red1,
    Red2,
    Red3,
    Red4,

    Purple1,
    Purple2,
    Purple3,
}

impl ColorRaw {
    pub const fn value(self) -> &'static str {
        match self {
            // Dark backgrounds
            Self::CharcoalDarkest => "#151413",
            Self::CharcoalDark => "#1E1C1B",
            Self::CharcoalMedium => "#262423",
            Self::CharcoalLight => "#37332E",

            // Warm text colors
            Self::TanMuted => "#C5B5A3",
            Self::TanWarm => "#DBD1B5",
            Self::BeigeWarm => "#E8DDD0",
            Self::BeigeLight => "#EBE1C6",
            Self::CreamWarm => "#EAE6DC",
            Self::CreamLight => "#F5F1E7",

            // Neutral extremes
            Self::NearBlack => "#11131A",
            Self::Whiteish => "#FAFAFA",

            // Transparent overlays
            Self::WarmOverlay30 => "rgba(231, 212, 198, 0.30)",
            Self::DarkOverlay70 => "rgba(30, 28, 27, 0.80)",
            Self::DarkOverlay40 => "rgba(55, 51, 46, 0.4)",
            Self::Black15 => "rgba(0, 0, 0, 0.15)",

            Self::Red1 => "#991b1b",
            Self::Red2 => "#b91c1c",
            Self::Red3 => "#dc2626",
            Self::Red4 => "#ef4444",
            Self::Purple1 => "#7e22ce",
            Self::Purple2 => "#9333ea",
            Self::Purple3 => "#a855f7",
        }
    }
}
