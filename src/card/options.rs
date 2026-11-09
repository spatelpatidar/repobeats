use serde::{Deserialize, Deserializer, de::Error as _};
use thiserror::Error;

use super::colors::ColorScheme;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum CardTheme {
    #[default]
    Light,
    Dark,
    GithubDimmed,
    Dracula,
    Nord,
    TokyoNight,
    SolarizedLight,
}

impl CardTheme {
    pub const ALL: [Self; 7] = [
        Self::Light,
        Self::Dark,
        Self::GithubDimmed,
        Self::Dracula,
        Self::Nord,
        Self::TokyoNight,
        Self::SolarizedLight,
    ];
}

/// Which card to draw.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum CardKind {
    /// Issues, pull requests, pushes and top contributors (the original card).
    #[default]
    Activity,
    /// Language breakdown by bytes of code.
    Languages,
    /// Ranked contributors with a bar per person.
    Contributors,
    /// Stars, forks, issues, license, last push and latest release.
    Overview,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CardSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl CardSize {
    pub fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Small => (610, 239),
            Self::Medium => (814, 318),
            Self::Large => (1_221, 477),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PeriodDays(u16);

impl PeriodDays {
    pub const SEVEN: Self = Self(7);
    pub const FOURTEEN: Self = Self(14);
    pub const THIRTY: Self = Self(30);
    pub const NINETY: Self = Self(90);
    pub const ONE_EIGHTY: Self = Self(180);
    pub const YEAR: Self = Self(365);

    pub fn get(self) -> usize {
        usize::from(self.0)
    }

    pub(super) fn comparison_label(self) -> &'static str {
        match self {
            Self::SEVEN => "past 7 days",
            Self::FOURTEEN => "past 14 days",
            Self::THIRTY => "past month",
            Self::NINETY => "past 90 days",
            Self::ONE_EIGHTY => "past 180 days",
            Self::YEAR => "past year",
            _ => unreachable!("PeriodDays can only contain a validated period"),
        }
    }
}

impl Default for PeriodDays {
    fn default() -> Self {
        Self::THIRTY
    }
}

impl TryFrom<u16> for PeriodDays {
    type Error = PeriodDaysError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            7 => Ok(Self::SEVEN),
            14 => Ok(Self::FOURTEEN),
            30 => Ok(Self::THIRTY),
            90 => Ok(Self::NINETY),
            180 => Ok(Self::ONE_EIGHTY),
            365 => Ok(Self::YEAR),
            _ => Err(PeriodDaysError(value)),
        }
    }
}

impl<'de> Deserialize<'de> for PeriodDays {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(u16::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[error("period must be 7, 14, 30, 90, 180, or 365 days, got {0}")]
pub struct PeriodDaysError(u16);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct CardOptions {
    #[serde(default)]
    pub kind: CardKind,
    #[serde(default)]
    pub theme: CardTheme,
    #[serde(default)]
    pub size: CardSize,
    #[serde(default)]
    pub period: PeriodDays,
    /// Custom colors. When set, they replace the colors of `theme`.
    /// Position decides the role, see [`ColorScheme`].
    #[serde(default)]
    pub colors: Option<ColorScheme>,
    /// Play the entrance and looping animations. Viewers with
    /// `prefers-reduced-motion` see the finished card either way.
    #[serde(default = "animate_default")]
    pub animate: bool,
}

fn animate_default() -> bool {
    true
}

impl Default for CardOptions {
    fn default() -> Self {
        Self {
            kind: CardKind::default(),
            theme: CardTheme::default(),
            size: CardSize::default(),
            period: PeriodDays::default(),
            colors: None,
            animate: true,
        }
    }
}
