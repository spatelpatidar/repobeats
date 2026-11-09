//! Custom color schemes: a short list of hex codes where the position in the
//! list decides the role.
//!
//! | Index | Role           | Used for                                                       |
//! | ----- | -------------- | -------------------------------------------------------------- |
//! | 0     | `background`   | Card background (`transparent` is allowed). Panels and borders are derived from it. |
//! | 1     | `primary`      | Titles, header icon and day strip, Issues series, contributor names and bars. |
//! | 2     | `secondary`    | Pull Requests series.                                          |
//! | 3     | `text`         | Values and labels. Muted text is derived from it.              |
//! | 4     | `tertiary`     | Pushes series. Optional: derived from `primary` when omitted.  |

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, de::Error as _};
use thiserror::Error;

/// Role of each position in the color list, in order. The last one is optional.
pub const COLOR_ROLES: [&str; 5] = ["background", "primary", "secondary", "text", "tertiary"];

const ROLE_LIST: &str =
    "[0] background, [1] primary, [2] secondary, [3] text, [4] tertiary (optional)";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    const WHITE: Self = Self(255, 255, 255);

    fn parse(value: &str) -> Option<Self> {
        let digits = value.strip_prefix('#').unwrap_or(value);
        if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |range: std::ops::Range<usize>| u8::from_str_radix(&digits[range], 16).ok();
        match digits.len() {
            3 => {
                let nibble = |i: usize| byte(i..i + 1).map(|n| n * 17);
                Some(Self(nibble(0)?, nibble(1)?, nibble(2)?))
            }
            6 => Some(Self(byte(0..2)?, byte(2..4)?, byte(4..6)?)),
            _ => None,
        }
    }

    /// `self` moved `amount` (0.0 to 1.0) of the way towards `other`.
    pub fn mix(self, other: Self, amount: f64) -> Self {
        let channel = |a: u8, b: u8| {
            (f64::from(a) + (f64::from(b) - f64::from(a)) * amount.clamp(0.0, 1.0)).round() as u8
        };
        Self(
            channel(self.0, other.0),
            channel(self.1, other.1),
            channel(self.2, other.2),
        )
    }

    pub fn lighten(self, amount: f64) -> Self {
        self.mix(Self::WHITE, amount)
    }

    /// Relative luminance, 0.0 (black) to 1.0 (white).
    pub fn luminance(self) -> f64 {
        let linear = |channel: u8| {
            let value = f64::from(channel) / 255.0;
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(self.0) + 0.7152 * linear(self.1) + 0.0722 * linear(self.2)
    }

    pub fn rgba(self, alpha: f64) -> String {
        format!("rgba({},{},{},{alpha})", self.0, self.1, self.2)
    }

    /// Same color with the hue turned by `degrees`.
    pub fn rotate_hue(self, degrees: f64) -> Self {
        let (r, g, b) = (
            f64::from(self.0) / 255.0,
            f64::from(self.1) / 255.0,
            f64::from(self.2) / 255.0,
        );
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let lightness = (max + min) / 2.0;
        let delta = max - min;
        if delta == 0.0 {
            return self;
        }
        let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
        let hue = if max == r {
            ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            (b - r) / delta + 2.0
        } else {
            (r - g) / delta + 4.0
        } * 60.0;
        let hue = (hue + degrees).rem_euclid(360.0);
        let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
        let x = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
        let (r1, g1, b1) = match (hue / 60.0) as u8 {
            0 => (chroma, x, 0.0),
            1 => (x, chroma, 0.0),
            2 => (0.0, chroma, x),
            3 => (0.0, x, chroma),
            4 => (x, 0.0, chroma),
            _ => (chroma, 0.0, x),
        };
        let offset = lightness - chroma / 2.0;
        let byte = |v: f64| ((v + offset) * 255.0).round().clamp(0.0, 255.0) as u8;
        Self(byte(r1), byte(g1), byte(b1))
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ColorError {
    #[error("colors needs 4 or 5 values ({ROLE_LIST}), got {0}")]
    Count(usize),
    #[error("colors[{index}] ({role}) is `{value}`: use #rgb or #rrggbb")]
    Invalid {
        index: usize,
        role: &'static str,
        value: String,
    },
    #[error("colors[{index}] ({role}) cannot be transparent: only colors[0] (background) can")]
    Transparent { index: usize, role: &'static str },
}

/// Colors for a card, resolved from a list where the position sets the role.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ColorScheme {
    /// `colors[0]`. `None` means transparent.
    pub background: Option<Rgb>,
    /// `colors[1]`.
    pub primary: Rgb,
    /// `colors[2]`.
    pub secondary: Rgb,
    /// `colors[3]`.
    pub text: Rgb,
    /// `colors[4]`, when given.
    pub tertiary: Option<Rgb>,
}

impl ColorScheme {
    /// Builds a scheme from 4 or 5 color strings in role order.
    pub fn from_list<S: AsRef<str>>(list: &[S]) -> Result<Self, ColorError> {
        if !(4..=5).contains(&list.len()) {
            return Err(ColorError::Count(list.len()));
        }
        let parse = |index: usize| -> Result<Rgb, ColorError> {
            let value = list[index].as_ref().trim();
            if value.eq_ignore_ascii_case("transparent") || value.eq_ignore_ascii_case("none") {
                return Err(ColorError::Transparent {
                    index,
                    role: COLOR_ROLES[index],
                });
            }
            Rgb::parse(value).ok_or_else(|| ColorError::Invalid {
                index,
                role: COLOR_ROLES[index],
                value: value.to_owned(),
            })
        };
        let background = {
            let value = list[0].as_ref().trim();
            if value.eq_ignore_ascii_case("transparent") || value.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(parse(0)?)
            }
        };
        Ok(Self {
            background,
            primary: parse(1)?,
            secondary: parse(2)?,
            text: parse(3)?,
            tertiary: if list.len() == 5 {
                Some(parse(4)?)
            } else {
                None
            },
        })
    }

    /// The tertiary color, or one derived from `primary` (hue turned 120 degrees).
    pub fn tertiary_or_derived(&self) -> Rgb {
        self.tertiary
            .unwrap_or_else(|| self.primary.rotate_hue(120.0))
    }

    /// True when the card is dark, judged by the background, or by the text
    /// color when the background is transparent.
    pub fn is_dark(&self) -> bool {
        match self.background {
            Some(background) => background.luminance() < self.text.luminance(),
            None => self.text.luminance() > 0.5,
        }
    }

    /// One line saying which position went where, written into the SVG as a comment.
    pub fn describe(&self) -> String {
        let background = self
            .background
            .map_or_else(|| "transparent".to_owned(), |c| c.to_string());
        let tertiary = match self.tertiary {
            Some(color) => color.to_string(),
            None => format!("{} (derived)", self.tertiary_or_derived()),
        };
        format!(
            "repobeats colors: [0] background={background}, [1] primary={}, [2] secondary={}, [3] text={}, [4] tertiary={tertiary}",
            self.primary, self.secondary, self.text
        )
    }
}

/// Accepts `#0d1117,#58a6ff,#bc8cff,#e6edf3` (commas or spaces, `#` optional)
/// or a JSON array such as `["#0d1117", "#58a6ff", "#bc8cff", "#e6edf3"]`.
impl FromStr for ColorScheme {
    type Err = ColorError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        if trimmed.starts_with('[')
            && let Ok(list) = serde_json::from_str::<Vec<String>>(trimmed)
        {
            return Self::from_list(&list);
        }
        let list = trimmed
            .trim_matches(['[', ']'])
            .split(|c: char| c == ',' || c.is_whitespace())
            .map(|part| part.trim_matches(['"', '\'']))
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        Self::from_list(&list)
    }
}

impl<'de> Deserialize<'de> for ColorScheme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_list(&Vec::<String>::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_forms() {
        assert_eq!(Rgb::parse("#fff"), Some(Rgb(255, 255, 255)));
        assert_eq!(Rgb::parse("0D1117"), Some(Rgb(13, 17, 23)));
        assert_eq!(Rgb::parse("#12345"), None);
        assert_eq!(Rgb::parse("#gggggg"), None);
    }

    #[test]
    fn rotates_hue() {
        assert_eq!(Rgb(255, 0, 0).rotate_hue(120.0), Rgb(0, 255, 0));
        assert_eq!(Rgb(128, 128, 128).rotate_hue(120.0), Rgb(128, 128, 128));
    }
}
