//! Options for the contribution graph card (`card: graph`, or `/graph` on the
//! server).
//!
//! | Parameter      | Meaning                                              | Default          |
//! | -------------- | ---------------------------------------------------- | ---------------- |
//! | `days`         | Days to plot, 1 to 365                               | `31`             |
//! | `from`, `to`   | `YYYY-MM-DD` range, inclusive. Wins over `days`      |                  |
//! | `metric`       | `contributions`, `pushes`, `issues`, `pulls`         | `contributions`  |
//! | `area`         | Fill under the line                                  | `false`          |
//! | `grid`         | Draw grid lines                                      | `true`           |
//! | `smooth`       | Curve the line instead of straight segments          | `true`           |
//! | `hide_title`   | Drop the title row                                   | `false`          |
//! | `custom_title` | Replace the title                                    |                  |
//! | `hide_border`  | No border                                            | `false`          |
//! | `radius`       | Corner radius, 0 to 16 (clamped)                     | `0`              |
//! | `height`       | Height in card units, 200 to 600 (clamped)           | `318`            |
//! | `bg_color`, `border_color`, `color`, `title_color`, `line`, `point`, `area_color` | Hex colors, `#` optional. `bg_color` also takes `transparent`. Each one replaces that part of the theme. | from the theme |

use chrono::NaiveDate;

use super::colors::Rgb;
use crate::{
    model::DailyActivity,
    params::{ParamError, invalid, parse_bool},
};

pub const DEFAULT_DAYS: u16 = 31;
pub const MAX_DAYS: u16 = 365;
pub const MIN_HEIGHT: u32 = 200;
pub const MAX_HEIGHT: u32 = 600;
pub const DEFAULT_HEIGHT: u32 = 318;
pub const MAX_RADIUS: u32 = 16;

/// What one point on the graph counts.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum GraphMetric {
    /// Pushes + issues opened + pull requests opened, as in the activity card header.
    #[default]
    Contributions,
    Pushes,
    Issues,
    PullRequests,
}

impl GraphMetric {
    pub fn value(self, day: &DailyActivity) -> u32 {
        match self {
            Self::Contributions => day.contributions(),
            Self::Pushes => day.pushes,
            Self::Issues => day.issues_opened,
            Self::PullRequests => day.pulls_opened,
        }
    }

    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Contributions => "Contribution Graph",
            Self::Pushes => "Push Graph",
            Self::Issues => "Issues Graph",
            Self::PullRequests => "Pull Request Graph",
        }
    }

    pub(super) fn unit(self, count: u32) -> &'static str {
        match (self, count) {
            (Self::Contributions, 1) => "contribution",
            (Self::Contributions, _) => "contributions",
            (Self::Pushes, 1) => "push",
            (Self::Pushes, _) => "pushes",
            (Self::Issues, 1) => "issue opened",
            (Self::Issues, _) => "issues opened",
            (Self::PullRequests, 1) => "pull request opened",
            (Self::PullRequests, _) => "pull requests opened",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "contributions" | "contribution" | "all" => Some(Self::Contributions),
            "pushes" | "push" | "commits" => Some(Self::Pushes),
            "issues" | "issue" => Some(Self::Issues),
            "pulls" | "pull" | "prs" | "pull-requests" | "pull_requests" => {
                Some(Self::PullRequests)
            }
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
    pub metric: GraphMetric,
    pub area: bool,
    pub grid: bool,
    pub smooth: bool,
    pub hide_title: bool,
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
            metric: GraphMetric::default(),
            area: false,
            grid: true,
            smooth: true,
            hide_title: false,
            hide_border: false,
            title: None,
            radius: 0,
            height: DEFAULT_HEIGHT,
            colors: GraphColors::default(),
        }
    }
}

impl GraphOptions {
    /// How many days back from `today` the range reaches, counting `today`.
    pub fn lookback_days(&self, today: NaiveDate) -> i64 {
        match self.range {
            GraphRange::Last(days) => i64::from(days),
            GraphRange::Between { from, .. } => (today - from).num_days() + 1,
        }
    }

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
                "metric" => {
                    options.metric = GraphMetric::parse(value).ok_or_else(|| {
                        invalid(
                            "metric",
                            value,
                            "use contributions, pushes, issues or pulls",
                        )
                    })?;
                }
                "area" => options.area = parse_bool("area", value)?,
                "grid" => options.grid = parse_bool("grid", value)?,
                "smooth" => options.smooth = parse_bool("smooth", value)?,
                "hide_title" => options.hide_title = parse_bool("hide_title", value)?,
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
