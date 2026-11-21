//! Options for the contribution graph card (`card: graph`, or `/graph` on the
//! server). The graph is of a GitHub profile: the options say which days to
//! show and how it looks, the user comes from `username`.
//!
//! | Parameter      | Meaning                                              | Default          |
//! | -------------- | ---------------------------------------------------- | ---------------- |
//! | `days`         | Days to plot, 1 to 365                               | `31`             |
//! | `from`, `to`   | `YYYY-MM-DD` range, inclusive. Wins over `days`      |                  |
//! | `area`         | Fill under the line                                  | `false`          |
//! | `grid`         | Draw grid lines                                      | `true`           |
//! | `smooth`       | Curve the line instead of straight segments          | `true`           |
//! | `hide_title`   | Drop the title row                                   | `false`          |
//! | `custom_title` | Replace the title                                    |                  |
//! | `labels`       | x-axis labels: `auto` (day numbers), `date`, `number`, `sideways`, `none` | `auto` |
//! | `hide_axis_titles` | Drop the "Days" and "Contributions" axis titles | `false`          |
//! | `hide_summary` | Drop the "520 contributions · Aug 23 – Nov 20" row   | `false`          |
//! | `hide_border`  | No border                                            | `false`          |
//! | `radius`       | Corner radius, 0 to 16 (clamped)                     | `0`              |
//! | `height`       | Height in card units, 200 to 600 (clamped)           | `420`            |
//! | `bg_color`, `border_color`, `color`, `title_color`, `line`, `point`, `area_color` | Hex colors, `#` optional. `bg_color` also takes `transparent`. Each one replaces that part of the theme. | from the theme |

use chrono::NaiveDate;

use super::colors::Rgb;
use crate::params::{ParamError, invalid, parse_bool};

pub const DEFAULT_DAYS: u16 = 31;
pub const MAX_DAYS: u16 = 365;
pub const MIN_HEIGHT: u32 = 200;
pub const MAX_HEIGHT: u32 = 600;
pub const DEFAULT_HEIGHT: u32 = 420;
pub const MAX_RADIUS: u32 = 16;

/// How the days are written under the x axis.
///
/// Every mode labels every day, as github-readme-activity-graph does, so on
/// long ranges the labels run together.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LabelMode {
    /// The day of the month, like the reference. Same as `Number`.
    #[default]
    Auto,
    /// Horizontal dates such as `Nov 9`.
    Date,
    /// The day of the month: `14`, `15`, `16`.
    Number,
    /// Dates written sideways, one under every day.
    Sideways,
    /// No labels under the axis.
    None,
}

impl LabelMode {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "date" | "dates" => Some(Self::Date),
            "number" | "numbers" | "day" | "days" => Some(Self::Number),
            "sideways" | "rotated" | "vertical" => Some(Self::Sideways),
            "none" | "off" | "hide" => Some(Self::None),
            _ => None,
        }
    }
}

/// Which days to plot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphRange {
    /// The last `n` days, ending today.
    Last(u16),
    /// An inclusive date range.
    Between { from: NaiveDate, to: NaiveDate },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Background {
    Transparent,
    Solid(Rgb),
}

/// Per-part color overrides. `None` keeps the color of the theme.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GraphColors {
    pub background: Option<Background>,
    pub border: Option<Rgb>,
    pub text: Option<Rgb>,
    pub title: Option<Rgb>,
    pub line: Option<Rgb>,
    pub point: Option<Rgb>,
    pub area: Option<Rgb>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphOptions {
    pub range: GraphRange,
    pub area: bool,
    pub grid: bool,
    pub smooth: bool,
    pub hide_title: bool,
    pub labels: LabelMode,
    pub hide_axis_titles: bool,
    pub hide_summary: bool,
    pub hide_border: bool,
    pub title: Option<String>,
    pub radius: u32,
    pub height: u32,
    pub colors: GraphColors,
}

impl Default for GraphOptions {
    fn default() -> Self {
        Self {
            range: GraphRange::Last(DEFAULT_DAYS),
            area: false,
            grid: true,
            smooth: true,
            hide_title: false,
            labels: LabelMode::Auto,
            hide_axis_titles: false,
            hide_summary: false,
            hide_border: false,
            title: None,
            radius: 0,
            height: DEFAULT_HEIGHT,
            colors: GraphColors::default(),
        }
    }
}

impl GraphOptions {
    /// Reads a URL query string such as `days=14&area=true&line=58a6ff`.
    pub fn from_query(query: &str) -> Result<Self, ParamError> {
        Self::from_pairs(form_urlencoded::parse(query.as_bytes()))
    }
}

const HEX: &str = "use a hex color such as 58a6ff or #58a6ff";
const DATE: &str = "use YYYY-MM-DD";

impl GraphOptions {
    /// Reads graph parameters. Names are case-insensitive, empty values count
    /// as unset, and names this card does not know are ignored (so
    /// cache-busting parameters such as `v=2` are harmless).
    ///
    /// Numbers that cannot be read are an error. `radius` and `height` are
    /// clamped to their range, `days` must be within 1 to 365.
    pub fn from_pairs<I, K, V>(pairs: I) -> Result<Self, ParamError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut options = Self::default();
        let (mut from, mut to) = (None, None);
        for (key, value) in pairs {
            let value = value.as_ref().trim();
            if value.is_empty() {
                continue;
            }
            match key.as_ref().to_ascii_lowercase().as_str() {
                "days" => {
                    let days = value
                        .parse::<u16>()
                        .ok()
                        .filter(|days| (1..=MAX_DAYS).contains(days))
                        .ok_or_else(|| invalid("days", value, "use a whole number, 1 to 365"))?;
                    options.range = GraphRange::Last(days);
                }
                "from" => from = Some(date("from", value)?),
                "to" => to = Some(date("to", value)?),
                "area" => options.area = parse_bool("area", value)?,
                "grid" => options.grid = parse_bool("grid", value)?,
                "smooth" => options.smooth = parse_bool("smooth", value)?,
                "hide_title" => options.hide_title = parse_bool("hide_title", value)?,
                "labels" => {
                    options.labels = LabelMode::parse(value).ok_or_else(|| {
                        invalid("labels", value, "use auto, date, number, sideways or none")
                    })?;
                }
                "hide_axis_titles" => {
                    options.hide_axis_titles = parse_bool("hide_axis_titles", value)?;
                }
                "hide_summary" => options.hide_summary = parse_bool("hide_summary", value)?,
                "hide_border" => options.hide_border = parse_bool("hide_border", value)?,
                "custom_title" => options.title = Some(value.to_owned()),
                "radius" => {
                    options.radius = value
                        .parse::<u32>()
                        .map_err(|_| invalid("radius", value, "use a whole number, 0 to 16"))?
                        .min(MAX_RADIUS);
                }
                "height" => {
                    options.height = value
                        .parse::<u32>()
                        .map_err(|_| invalid("height", value, "use a whole number, 200 to 600"))?
                        .clamp(MIN_HEIGHT, MAX_HEIGHT);
                }
                "bg_color" => {
                    options.colors.background =
                        Some(if value.eq_ignore_ascii_case("transparent") {
                            Background::Transparent
                        } else {
                            Background::Solid(hex("bg_color", value)?)
                        });
                }
                "border_color" => options.colors.border = Some(hex("border_color", value)?),
                "color" => options.colors.text = Some(hex("color", value)?),
                "title_color" => options.colors.title = Some(hex("title_color", value)?),
                "line" => options.colors.line = Some(hex("line", value)?),
                "point" => options.colors.point = Some(hex("point", value)?),
                "area_color" => options.colors.area = Some(hex("area_color", value)?),
                _ => {}
            }
        }
        match (from, to) {
            (Some(from), Some(to)) if from <= to => {
                options.range = GraphRange::Between { from, to };
            }
            (Some(_), Some(_)) => {
                return Err(ParamError::Conflict("`from` must not be after `to`"));
            }
            (Some(_), None) => return Err(ParamError::Missing("to")),
            (None, Some(_)) => return Err(ParamError::Missing("from")),
            (None, None) => {}
        }
        Ok(options)
    }
}

fn hex(name: &'static str, value: &str) -> Result<Rgb, ParamError> {
    Rgb::parse(value).ok_or_else(|| invalid(name, value, HEX))
}

fn date(name: &'static str, value: &str) -> Result<NaiveDate, ParamError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| invalid(name, value, DATE))
}
