//! The contribution graph: one line (optionally filled) of daily counts over
//! a date range, in the style of github-readme-activity-graph but drawn as a
//! plain SVG with the same palette, fonts and animation as the other cards.
//!
//! Everything dynamic is either a number, a date, a validated color or text
//! that goes through `text`, which escapes it.

use std::fmt::Write as _;

use chrono::{Datelike, Duration, NaiveDate};
use svg::{
    Document,
    node::{
        Blob,
        element::{Rectangle, Style, Text},
    },
};
use thiserror::Error;

use super::{CardOptions, Palette, color_comment, extra::ellipsize, style, wave};
use crate::{
    card::graph_options::{Background, GraphColors, GraphOptions, GraphRange},
    model::ActivitySnapshot,
};

const WIDTH: f64 = 814.0;
const LEFT: f64 = 52.0;
const RIGHT: f64 = WIDTH - 24.0;
const BOTTOM_PAD: f64 = 40.0;
const TITLE_TOP: f64 = 62.0;
const NO_TITLE_TOP: f64 = 28.0;
/// Points are drawn up to this many days; beyond it the line is enough.
const MAX_POINTS: usize = 60;
/// Longest custom title, in characters.
const TITLE_LIMIT: usize = 64;

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum GraphError {
    #[error("the snapshot has no data for the last {0} days")]
    NotEnoughHistory(usize),
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

pub fn render_graph(
    snapshot: &ActivitySnapshot,
    card: CardOptions,
    graph: &GraphOptions,
) -> Result<String, GraphError> {
    let days = select(snapshot, graph)?;
    let palette = Palette::resolve(&card);
    let colors = Colors::resolve(&palette, &graph.colors, graph.hide_border);

    let height = f64::from(graph.height);
    let plot_top = if graph.hide_title {
        NO_TITLE_TOP
    } else {
        TITLE_TOP
    };
    let plot_bottom = height - BOTTOM_PAD;
    let plot_height = plot_bottom - plot_top;
    let max = days.iter().map(|(_, value)| *value).max().unwrap_or(0);
    let (step, top) = nice_scale(max);
    let count = days.len();

    let x_at = |index: usize| {
        if count == 1 {
            (LEFT + RIGHT) / 2.0
        } else {
            LEFT + (RIGHT - LEFT) * index as f64 / (count - 1) as f64
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
            None => format!("{} {}", snapshot.repository, graph.metric.title()),
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
            graph.metric.unit(total),
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
                r#"<line x1="{LEFT}" y1="{y:.1}" x2="{RIGHT}" y2="{y:.1}" stroke="{}" stroke-opacity="{}" stroke-width="1"/>"#,
                colors.text,
                if tick == 0 { 0.4 } else { 0.15 },
            );
        }
        text(
            &mut body,
            Anchor::End,
            (LEFT - 10.0, y + 4.0),
            "sans smaller",
            &colors.text,
            &value.to_string(),
        );
    }

    // Date axis: label the latest day and every `stride` days before it.
    let multi_year = days[0].0.year() != days[count - 1].0.year();
    let spacing = if multi_year { 78.0 } else { 56.0 };
    let slot = if count > 1 {
        (RIGHT - LEFT) / (count - 1) as f64
    } else {
        RIGHT - LEFT
    };
    let stride = (spacing / slot).ceil().max(1.0) as usize;
    let mut index = count - 1;
    loop {
        let x = x_at(index);
        if graph.grid && count > 1 {
            let _ = write!(
                body,
                r#"<line x1="{x:.1}" y1="{plot_top}" x2="{x:.1}" y2="{plot_bottom}" stroke="{}" stroke-opacity="0.15" stroke-width="1"/>"#,
                colors.text
            );
        }
        text(
            &mut body,
            Anchor::Middle,
            (x, plot_bottom + 22.0),
            "sans smaller",
            &colors.text,
            &short_date(days[index].0, multi_year),
        );
        match index.checked_sub(stride) {
            Some(next) => index = next,
            None => break,
        }
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
    if count <= MAX_POINTS {
        for (index, ((date, value), (x, y))) in days.iter().zip(&points).enumerate() {
            let _ = write!(
                body,
                r#"<circle class="cell w{}" cx="{x:.1}" cy="{y:.1}" r="3.5" fill="{}" stroke="{}" stroke-width="1.5"><title>{}: {value}</title></circle>"#,
                wave(index, count),
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

/// The line draws itself, the area fades in after it. Reduced-motion viewers
/// get the finished graph from the rule in `animation_css`.
const GRAPH_ANIMATION: &str = ".draw{stroke-dasharray:1;animation:draw 1.1s cubic-bezier(.2,.8,.2,1) .15s both}@keyframes draw{from{stroke-dashoffset:1}}.sweep{animation:fade .8s ease-out .7s both}";

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

/// `(date, value)` for every day of the requested range, oldest first.
fn select(
    snapshot: &ActivitySnapshot,
    graph: &GraphOptions,
) -> Result<Vec<(NaiveDate, u32)>, GraphError> {
    let available = snapshot.current.len();
    let latest = snapshot.generated_at.date_naive();
    let earliest = latest - Duration::days(available.saturating_sub(1) as i64);
    let (from, to) = match graph.range {
        GraphRange::Last(days) => {
            let days = usize::from(days);
            if days > available {
                return Err(GraphError::NotEnoughHistory(days));
            }
            (latest - Duration::days(days as i64 - 1), latest)
        }
        GraphRange::Between { from, to } => {
            if available == 0 || from < earliest || to > latest {
                return Err(GraphError::OutsideWindow { earliest, latest });
            }
            (from, to)
        }
    };
    let start = (from - earliest).num_days() as usize;
    let length = (to - from).num_days() as usize + 1;
    Ok(snapshot.current[start..start + length]
        .iter()
        .enumerate()
        .map(|(offset, day)| {
            (
                from + Duration::days(offset as i64),
                graph.metric.value(day),
            )
        })
        .collect())
}

/// Tick step and axis top for a maximum value: whole-number ticks, about five
/// intervals. `nice_scale(0)` is `(1, 4)` so an empty graph still has a scale.
fn nice_scale(max: u32) -> (u32, u32) {
    if max == 0 {
        return (1, 4);
    }
    let raw = f64::from(max) / 5.0;
    let magnitude = 10_f64.powf(raw.log10().floor().max(0.0));
    let step = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|factor| factor * magnitude)
        .find(|step| *step >= raw)
        .unwrap_or(10.0 * magnitude)
        .max(1.0) as u32;
    (step, max.div_ceil(step) * step)
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
    fn scale_uses_whole_numbers() {
        assert_eq!(nice_scale(0), (1, 4));
        assert_eq!(nice_scale(1), (1, 1));
        assert_eq!(nice_scale(3), (1, 3));
        assert_eq!(nice_scale(7), (2, 8));
        assert_eq!(nice_scale(100), (20, 100));
        assert_eq!(nice_scale(101), (50, 150));
        assert_eq!(nice_scale(1_234), (500, 1_500));
    }

    #[test]
    fn smooth_path_stays_between_neighbours() {
        let points = [(0.0, 100.0), (10.0, 100.0), (20.0, 40.0), (30.0, 100.0)];
        let path = smooth_path(&points);
        // Control points at a flat segment keep its height.
        assert!(path.starts_with("M0.0,100.0 C3.3,100.0 6.7,100.0 10.0,100.0"));
    }
}
