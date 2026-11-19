//! The contribution graph: one line (optionally filled) of daily counts over
//! a date range, in the style of github-readme-activity-graph but drawn as a
//! plain SVG with the same palette, fonts and animation as the other cards.
//!
//! Everything dynamic is either a number, a date, a validated color or text
//! that goes through `text`, which escapes it.

use std::{collections::HashMap, fmt::Write as _};

use chrono::{Datelike, Duration, NaiveDate};
use svg::{
    Document,
    node::{
        Blob,
        element::{Rectangle, Style, Text},
    },
};
use thiserror::Error;

use super::{CardOptions, Palette, color_comment, extra::ellipsize, style};
use crate::{
    card::graph_options::{Background, GraphColors, GraphOptions, GraphRange, LabelMode},
    model::ContributionCalendar,
};

const WIDTH: f64 = 814.0;
/// Left edge of the plot, without and with the vertical axis title.
const LEFT: f64 = 52.0;
const LEFT_TITLED: f64 = 66.0;
const RIGHT: f64 = WIDTH - 24.0;
const BOTTOM_PAD: f64 = 40.0;
/// Extra room under the date labels for the horizontal axis title.
const X_TITLE_PAD: f64 = 18.0;
/// Room under the plot when there are no date labels.
const NO_LABEL_PAD: f64 = 14.0;
/// Space one label needs along the axis, in each style, when labels must be spaced out.
const NUMBER_SPACING: f64 = 14.0;
const SIDEWAYS_SPACING: f64 = 9.0;
/// Room under the plot for dates written sideways.
const ROTATED_PAD: f64 = 62.0;
const TITLE_TOP: f64 = 62.0;
const NO_TITLE_TOP: f64 = 28.0;
/// Width of one day on the axis (card units) and what is written under it.
/// Wide slots get a full date ("Nov 9"), then the day of the month, then the
/// day of the month in smaller type, then the date written sideways. Below
/// `ROTATED_MIN_SLOT` one label per day cannot fit, and every few days get one.
const FULL_DATE_SLOT: f64 = 50.0;
const DAY_NUMBER_SLOT: f64 = 21.0;
const SMALL_NUMBER_SLOT: f64 = 12.0;
const ROTATED_MIN_SLOT: f64 = 7.5;
/// A vertical grid line for every day, and a point for every day, down to
/// these widths. Beyond that the grid follows the labels and the line has no points.
const GRID_MIN_SLOT: f64 = 5.0;
const POINT_MIN_SLOT: f64 = 6.0;
/// Longest custom title, in characters.
const TITLE_LIMIT: usize = 64;

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum GraphError {
    #[error("dates must fall between {earliest} and {latest}")]
    OutsideWindow {
        earliest: NaiveDate,
        latest: NaiveDate,
    },
}

/// Colors for each part of the graph: the theme first, then the overrides.
struct Colors {
    background: String,
    border: String,
    text: String,
    title: String,
    line: String,
    point: String,
    area: String,
}

impl Colors {
    fn resolve(palette: &Palette, over: &GraphColors, hide_border: bool) -> Self {
        let pick = |color: Option<super::Rgb>, fallback: &str| {
            color.map_or_else(|| fallback.to_owned(), |color| color.to_string())
        };
        Self {
            background: match over.background {
                Some(Background::Transparent) => "transparent".to_owned(),
                Some(Background::Solid(color)) => color.to_string(),
                None => palette.background.to_string(),
            },
            border: if hide_border {
                "none".to_owned()
            } else {
                pick(over.border, &palette.panel_stroke)
            },
            text: pick(over.text, &palette.text),
            title: pick(over.title, &palette.accent),
            line: pick(over.line, &palette.issues.main),
            point: pick(over.point, &palette.accent),
            area: pick(over.area, &palette.issues.main),
        }
    }
}

/// Draws the contribution graph of `calendar`. `today` (UTC) is the last day
/// of a `days=N` range.
pub fn render_graph(
    calendar: &ContributionCalendar,
    today: NaiveDate,
    card: CardOptions,
    graph: &GraphOptions,
) -> Result<String, GraphError> {
    let days = select(calendar, today, graph)?;
    let palette = Palette::resolve(&card);
    let colors = Colors::resolve(&palette, &graph.colors, graph.hide_border);

    let height = f64::from(graph.height);
    let plot_top = if graph.hide_title {
        NO_TITLE_TOP
    } else {
        TITLE_TOP
    };
    let count = days.len();
    let titled = !graph.hide_axis_titles;
    let left = if titled { LEFT_TITLED } else { LEFT };
    // Width of one day, which decides how the date axis is written.
    let slot = if count > 1 {
        (RIGHT - left) / (count - 1) as f64
    } else {
        RIGHT - left
    };
    let sideways = match graph.labels {
        LabelMode::Auto => (ROTATED_MIN_SLOT..SMALL_NUMBER_SLOT).contains(&slot),
        LabelMode::Sideways => true,
        _ => false,
    };
    let label_pad = match graph.labels {
        LabelMode::None => NO_LABEL_PAD,
        _ if sideways => ROTATED_PAD,
        _ => BOTTOM_PAD,
    };
    let plot_bottom = height - label_pad - if titled { X_TITLE_PAD } else { 0.0 };
    let plot_height = plot_bottom - plot_top;
    let max = days.iter().map(|(_, value)| *value).max().unwrap_or(0);
    let (step, top) = nice_scale(max, plot_height);

    let x_at = |index: usize| {
        if count == 1 {
            (left + RIGHT) / 2.0
        } else {
            left + (RIGHT - left) * index as f64 / (count - 1) as f64
        }
    };
    let y_at = |value: u32| plot_bottom - plot_height * f64::from(value) / f64::from(top);
    let points = days
        .iter()
        .enumerate()
        .map(|(index, (_, value))| (x_at(index), y_at(*value)))
        .collect::<Vec<_>>();

    let mut body = String::with_capacity(8_000);
    let _ = write!(
        body,
        r#"<defs><linearGradient id="repobeats-area" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{0}" stop-opacity="0.4"/><stop offset="1" stop-color="{0}" stop-opacity="0"/></linearGradient></defs>"#,
        colors.area
    );

    if !graph.hide_title {
        let total: u32 = days.iter().map(|(_, value)| value).sum();
        let title = match &graph.title {
            Some(title) => ellipsize(title, TITLE_LIMIT),
            None => format!(
                "{}'s Contribution Graph",
                calendar.name.as_deref().unwrap_or(&calendar.login)
            ),
        };
        text(
            &mut body,
            Anchor::Start,
            (24.0, 36.0),
            "sans small bold",
            &colors.title,
            &title,
        );
        let summary = format!(
            "{total} {} · {} – {}",
            if total == 1 {
                "contribution"
            } else {
                "contributions"
            },
            short_date(days[0].0, false),
            short_date(days[count - 1].0, false),
        );
        text(
            &mut body,
            Anchor::End,
            (RIGHT + 14.0, 36.0),
            "sans smaller",
            &colors.text,
            &summary,
        );
    }

    // Horizontal grid and the value axis.
    let ticks = top / step;
    for tick in 0..=ticks {
        let value = tick * step;
        let y = y_at(value);
        if graph.grid || tick == 0 {
            let _ = write!(
                body,
                r#"<line x1="{left}" y1="{y:.1}" x2="{RIGHT}" y2="{y:.1}" stroke="{}" stroke-opacity="{}" stroke-width="1"/>"#,
                colors.text,
                if tick == 0 { 0.4 } else { 0.15 },
            );
        }
        text(
            &mut body,
            Anchor::End,
            (left - 10.0, y + 4.0),
            "sans smaller",
            &colors.text,
            &value.to_string(),
        );
    }

    // Date axis: a vertical grid line and a label for every day while they
    // fit. Longer ranges label the latest day and every `stride` days before it.
    let multi_year = days[0].0.year() != days[count - 1].0.year();
    let spaced = |spacing: f64| (spacing / slot).ceil().max(1.0) as usize;
    let date_spacing = if multi_year { 78.0 } else { 56.0 };
    let number_class = if slot >= DAY_NUMBER_SLOT {
        "sans smaller"
    } else {
        "sans tiny"
    };
    let (stride, style_of): (usize, LabelStyle) = match graph.labels {
        LabelMode::Auto => {
            if slot >= FULL_DATE_SLOT {
                (1, LabelStyle::Date)
            } else if slot >= DAY_NUMBER_SLOT {
                (1, LabelStyle::Number("sans smaller"))
            } else if slot >= SMALL_NUMBER_SLOT {
                (1, LabelStyle::Number("sans tiny"))
            } else if sideways {
                (1, LabelStyle::Sideways)
            } else {
                (spaced(date_spacing), LabelStyle::Date)
            }
        }
        LabelMode::Date => (spaced(date_spacing), LabelStyle::Date),
        LabelMode::Number => (spaced(NUMBER_SPACING), LabelStyle::Number(number_class)),
        LabelMode::Sideways => (spaced(SIDEWAYS_SPACING), LabelStyle::Sideways),
        LabelMode::None => (spaced(date_spacing), LabelStyle::None),
    };
    if graph.grid && count > 1 {
        let (every, opacity) = if slot >= GRID_MIN_SLOT {
            (1, if slot >= SMALL_NUMBER_SLOT { 0.15 } else { 0.1 })
        } else {
            (stride, 0.15)
        };
        for index in (0..count).rev().step_by(every) {
            let x = x_at(index);
            let _ = write!(
                body,
                r#"<line class="vgrid" x1="{x:.1}" y1="{plot_top}" x2="{x:.1}" y2="{plot_bottom}" stroke="{}" stroke-opacity="{opacity}" stroke-width="1"/>"#,
                colors.text
            );
        }
    }
    for index in (0..count).rev().step_by(stride) {
        let x = x_at(index);
        let date = days[index].0;
        match style_of {
            LabelStyle::Date => text(
                &mut body,
                Anchor::Middle,
                (x, plot_bottom + 22.0),
                "sans smaller",
                &colors.text,
                &short_date(date, multi_year),
            ),
            LabelStyle::Number(class) => text(
                &mut body,
                Anchor::Middle,
                (x, plot_bottom + 22.0),
                class,
                &colors.text,
                &date.day().to_string(),
            ),
            LabelStyle::None => {}
            LabelStyle::Sideways => sideways_text(
                &mut body,
                (x, plot_bottom + 8.0),
                &colors.text,
                &short_date(date, multi_year),
            ),
        }
    }

    // Axis titles: "Days" centered under the date labels, "Contributions"
    // turned sideways along the value axis.
    if titled {
        text(
            &mut body,
            Anchor::Middle,
            ((left + RIGHT) / 2.0, height - 12.0),
            "sans smaller bold",
            &colors.text,
            "Days",
        );
        let middle = plot_top + plot_height / 2.0;
        let node = Text::new("Contributions")
            .set("x", "16.0")
            .set("y", format!("{middle:.1}"))
            .set("class", "sans smaller bold")
            .set("fill", colors.text.as_str())
            .set("text-anchor", "middle")
            .set("transform", format!("rotate(-90 16.0 {middle:.1})"));
        body.push_str(&node.to_string());
    }

    // Area, line, points.
    let line = if graph.smooth && count > 2 {
        smooth_path(&points)
    } else {
        straight_path(&points)
    };
    if graph.area && count > 1 {
        let first = points[0];
        let last = points[count - 1];
        let _ = write!(
            body,
            r#"<path class="sweep" d="M{:.1},{plot_bottom:.1} L{} L{:.1},{plot_bottom:.1} Z" fill="url(#repobeats-area)"/>"#,
            first.0,
            line_from_first(&line),
            last.0,
        );
    }
    let draw = if card.animate { r#" class="draw""# } else { "" };
    if count > 1 {
        let _ = write!(
            body,
            r#"<path{draw} pathLength="1" d="{line}" fill="none" stroke="{}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>"#,
            colors.line
        );
    }
    if slot >= POINT_MIN_SLOT {
        let radius = (slot * 0.38).clamp(1.6, 3.5);
        let ring = if radius < 3.0 { 0.8 } else { 1.5 };
        for (index, ((date, value), (x, y))) in days.iter().zip(&points).enumerate() {
            let _ = write!(
                body,
                r#"<circle class="dot" style="animation-delay:{}ms" cx="{x:.1}" cy="{y:.1}" r="{radius:.1}" fill="{}" stroke="{}" stroke-width="{ring}"><title>{}: {value}</title></circle>"#,
                (DRAW_DELAY_MS + DRAW_MS * index as f64 / (count.max(2) - 1) as f64).round(),
                colors.point,
                if colors.background == "transparent" {
                    "none"
                } else {
                    &colors.background
                },
                short_date(*date, multi_year),
            );
        }
    }

    let (pixel_width, _) = card.size.dimensions();
    let pixel_height = (f64::from(pixel_width) * height / WIDTH).round() as u32;
    let mut css = style(card.animate);
    css.push_str(GRAPH_TEXT);
    if card.animate {
        css.push_str(GRAPH_ANIMATION);
    }
    Ok(Document::new()
        .set("width", pixel_width)
        .set("height", pixel_height)
        .set("viewBox", (0, 0, 814, graph.height))
        .set("fill", "none")
        .add(color_comment(&card))
        .add(Style::new(css))
        .add(
            Rectangle::new()
                .set("x", 0.5)
                .set("y", 0.5)
                .set("width", 813)
                .set("height", height - 1.0)
                .set("rx", graph.radius)
                .set("fill", colors.background)
                .set("stroke", colors.border),
        )
        .add(Blob::new(body))
        .to_string())
}

/// Smaller type for axes that label many days.
const GRAPH_TEXT: &str = ".tiny{font-size:9px}.micro{font-size:8px}";

/// The line draws itself, the area fades in after it. Reduced-motion viewers
/// get the finished graph from the rule in `animation_css`.
const GRAPH_ANIMATION: &str = ".draw{stroke-dasharray:1;animation:draw 2.4s linear .2s both}@keyframes draw{from{stroke-dashoffset:1}}.dot{transform-box:fill-box;transform-origin:center;animation:pop .5s cubic-bezier(.2,.8,.2,1) both}@keyframes pop{from{opacity:0;transform:scale(0)}}.sweep{animation:fade 1.2s ease-out 1.4s both}";

/// The line takes this long to draw (see `GRAPH_ANIMATION`), and each point
/// appears when the line reaches it.
const DRAW_MS: f64 = 2400.0;
const DRAW_DELAY_MS: f64 = 200.0;

/// A small card with a message, for requests that cannot be drawn.
pub fn render_error(message: &str) -> String {
    let mut body = String::new();
    text(
        &mut body,
        Anchor::Start,
        (24.0, 44.0),
        "sans small bold",
        "#cf3e3e",
        "repobeats",
    );
    text(
        &mut body,
        Anchor::Start,
        (24.0, 76.0),
        "sans small",
        "#777",
        &ellipsize(message, 90),
    );
    Document::new()
        .set("width", 610)
        .set("height", 100)
        .set("viewBox", (0, 0, 814, 134))
        .set("fill", "none")
        .add(Style::new(super::BASE_STYLE))
        .add(
            Rectangle::new()
                .set("x", 0.5)
                .set("y", 0.5)
                .set("width", 813)
                .set("height", 133)
                .set("rx", 6)
                .set("fill", "#fff8f8")
                .set("stroke", "#e8b4b4"),
        )
        .add(Blob::new(body))
        .to_string()
}

/// How many days of history a profile graph can show, counting today.
pub const WINDOW_DAYS: i64 = 365;

/// First and last day of the range, checked against the 365-day window that
/// ends `today`.
pub fn graph_dates(
    graph: &GraphOptions,
    today: NaiveDate,
) -> Result<(NaiveDate, NaiveDate), GraphError> {
    let earliest = today - Duration::days(WINDOW_DAYS - 1);
    match graph.range {
        GraphRange::Last(days) => Ok((today - Duration::days(i64::from(days) - 1), today)),
        GraphRange::Between { from, to } => {
            if from < earliest || to > today {
                Err(GraphError::OutsideWindow {
                    earliest,
                    latest: today,
                })
            } else {
                Ok((from, to))
            }
        }
    }
}

/// `(date, count)` for every day of the requested range, oldest first. Days
/// the calendar does not list count as zero.
fn select(
    calendar: &ContributionCalendar,
    today: NaiveDate,
    graph: &GraphOptions,
) -> Result<Vec<(NaiveDate, u32)>, GraphError> {
    let (from, to) = graph_dates(graph, today)?;
    let counts = calendar.days.iter().copied().collect::<HashMap<_, _>>();
    let length = (to - from).num_days();
    Ok((0..=length)
        .map(|offset| {
            let date = from + Duration::days(offset);
            (date, counts.get(&date).copied().unwrap_or(0))
        })
        .collect())
}

/// Closest two ticks of the value axis may be, in card units.
const MIN_TICK_SPACE: f64 = 18.0;

/// Tick step and axis top for the highest daily count on a plot `plot_height`
/// tall. The step is the smallest of 1, 2, 5, 10, 20, 50 ... that still leaves
/// `MIN_TICK_SPACE` between ticks, so the axis gets finer as the graph gets
/// taller and coarser as the numbers grow (the way Chartist, which
/// github-readme-activity-graph uses, sizes its axis). Ticks are whole
/// numbers. With no activity at all the axis is `(1, 4)`.
fn nice_scale(max: u32, plot_height: f64) -> (u32, u32) {
    if max == 0 {
        return (1, 4);
    }
    let max = u64::from(max);
    let mut magnitude = 1_u64;
    loop {
        for factor in [1, 2, 5] {
            let step = factor * magnitude;
            let intervals = max.div_ceil(step);
            if step >= max || plot_height / intervals as f64 >= MIN_TICK_SPACE {
                let clamp = |value: u64| u32::try_from(value).unwrap_or(u32::MAX);
                return (clamp(step), clamp(intervals * step));
            }
        }
        magnitude *= 10;
    }
}

/// `Nov 3`, or `Nov 3 '25` when the range spans years.
fn short_date(date: NaiveDate, with_year: bool) -> String {
    if with_year {
        date.format("%b %-d '%y").to_string()
    } else {
        date.format("%b %-d").to_string()
    }
}

fn straight_path(points: &[(f64, f64)]) -> String {
    let mut path = String::new();
    for (index, (x, y)) in points.iter().enumerate() {
        let _ = write!(path, "{}{x:.1},{y:.1} ", if index == 0 { "M" } else { "L" });
    }
    path.trim_end().to_owned()
}

/// The path without its leading `M`, to continue from another start point.
fn line_from_first(path: &str) -> &str {
    path.strip_prefix('M').unwrap_or(path)
}

/// Monotone cubic curve through the points (Fritsch-Carlson). It never swings
/// past a data point, so a smoothed line cannot dip below zero.
fn smooth_path(points: &[(f64, f64)]) -> String {
    let count = points.len();
    let slope = |i: usize| (points[i + 1].1 - points[i].1) / (points[i + 1].0 - points[i].0);
    let mut tangent = vec![0.0; count];
    tangent[0] = slope(0);
    tangent[count - 1] = slope(count - 2);
    for (i, value) in tangent.iter_mut().enumerate().take(count - 1).skip(1) {
        let (before, after) = (slope(i - 1), slope(i));
        *value = if before * after <= 0.0 {
            0.0
        } else {
            (before + after) / 2.0
        };
    }
    for i in 0..count - 1 {
        let delta = slope(i);
        if delta == 0.0 {
            tangent[i] = 0.0;
            tangent[i + 1] = 0.0;
            continue;
        }
        let (a, b) = (tangent[i] / delta, tangent[i + 1] / delta);
        let size = a * a + b * b;
        if size > 9.0 {
            let scale = 3.0 / size.sqrt();
            tangent[i] = scale * a * delta;
            tangent[i + 1] = scale * b * delta;
        }
    }
    let mut path = format!("M{:.1},{:.1}", points[0].0, points[0].1);
    for i in 0..count - 1 {
        let third = (points[i + 1].0 - points[i].0) / 3.0;
        let _ = write!(
            path,
            " C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
            points[i].0 + third,
            points[i].1 + tangent[i] * third,
            points[i + 1].0 - third,
            points[i + 1].1 - tangent[i + 1] * third,
            points[i + 1].0,
            points[i + 1].1,
        );
    }
    path
}

/// How the label of each day on the date axis is written.
#[derive(Clone, Copy)]
enum LabelStyle {
    None,
    Date,
    Number(&'static str),
    Sideways,
}

/// A date written bottom to top, hanging below the axis with its end at `(x, y)`.
fn sideways_text(svg: &mut String, (x, y): (f64, f64), fill: &str, content: &str) {
    // The baseline sits a little right of the day, so the glyphs are centered on it.
    let x = x + 3.0;
    let node = Text::new(content)
        .set("x", format!("{x:.1}"))
        .set("y", format!("{y:.1}"))
        .set("class", "sans micro")
        .set("fill", fill)
        .set("text-anchor", "end")
        .set("transform", format!("rotate(-90 {x:.1} {y:.1})"));
    svg.push_str(&node.to_string());
}

#[derive(Clone, Copy)]
enum Anchor {
    Start,
    Middle,
    End,
}

fn text(
    svg: &mut String,
    anchor: Anchor,
    (x, y): (f64, f64),
    class: &str,
    fill: &str,
    content: &str,
) {
    let mut node = Text::new(content)
        .set("x", format!("{x:.1}"))
        .set("y", format!("{y:.1}"))
        .set("class", class)
        .set("fill", fill);
    match anchor {
        Anchor::Start => {}
        Anchor::Middle => node = node.set("text-anchor", "middle"),
        Anchor::End => node = node.set("text-anchor", "end"),
    }
    svg.push_str(&node.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_gets_finer_with_room_and_coarser_with_big_numbers() {
        // The default plot is about 198 units tall: ticks at least 18 apart.
        let tall = 198.0;
        assert_eq!(nice_scale(0, tall), (1, 4));
        assert_eq!(nice_scale(1, tall), (1, 1));
        assert_eq!(nice_scale(5, tall), (1, 5));
        assert_eq!(nice_scale(10, tall), (1, 10));
        assert_eq!(nice_scale(12, tall), (2, 12));
        assert_eq!(nice_scale(15, tall), (2, 16));
        assert_eq!(nice_scale(20, tall), (2, 20));
        assert_eq!(nice_scale(100, tall), (10, 100));
        assert_eq!(nice_scale(101, tall), (10, 110));
        assert_eq!(nice_scale(1_234, tall), (200, 1_400));
        // A short plot has less room, so the same maximum gets a coarser step.
        assert_eq!(nice_scale(20, 80.0), (5, 20));
        assert_eq!(nice_scale(20, 400.0), (1, 20));
        // Huge counts do not overflow.
        assert!(nice_scale(u32::MAX, tall).0 > 1);
    }

    #[test]
    fn smooth_path_stays_between_neighbours() {
        let points = [(0.0, 100.0), (10.0, 100.0), (20.0, 40.0), (30.0, 100.0)];
        let path = smooth_path(&points);
        // Control points at a flat segment keep its height.
        assert!(path.starts_with("M0.0,100.0 C3.3,100.0 6.7,100.0 10.0,100.0"));
    }
}
