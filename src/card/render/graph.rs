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
    card::CardSize,
    card::graph_options::{Background, GraphColors, GraphOptions, GraphRange, LabelMode},
    model::ContributionCalendar,
};

/// The graph is laid out in 1200 x `height` units, like github-readme-activity-graph.
pub const WIDTH: f64 = 1200.0;
/// Space around the plot, as in the reference: the value axis labels and
/// title sit in `Y_OFFSET`, the date labels and title in `X_OFFSET`.
const PAD_TOP: f64 = 80.0;
const PAD_RIGHT: f64 = 50.0;
const PAD_BOTTOM: f64 = 20.0;
const PAD_LEFT: f64 = 20.0;
const Y_OFFSET: f64 = 70.0;
const X_OFFSET: f64 = 50.0;
/// Closest two value-axis ticks may be (the reference's `scaleMinSpace`).
const SCALE_MIN_SPACE: f64 = 20.0;
/// Rough widths of one title (20px, 600 weight) and summary (14px) character,
/// to tell whether the summary fits beside the title.
const TITLE_CHAR_WIDTH: f64 = 11.5;
const SUMMARY_CHAR_WIDTH: f64 = 7.4;
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

/// The graph colors of a theme as `(name, "#rrggbb")` pairs, for the params
/// page: `bg_color`, `border_color`, `title_color`, `color`, `line`, `point`
/// and `area_color`.
pub fn theme_colors(card: CardOptions) -> [(&'static str, String); 7] {
    let palette = Palette::resolve(&card);
    let colors = Colors::resolve(&palette, &GraphColors::default(), false);
    [
        ("bg_color", colors.background),
        ("border_color", colors.border),
        ("title_color", colors.title),
        ("color", colors.text),
        ("line", colors.line),
        ("point", colors.point),
        ("area_color", colors.area),
    ]
}

/// Draws the contribution graph of `calendar`. `today` (UTC) is the last day
/// of a `days=N` range.
///
/// The layout, the value-axis rule, the labels and the animation follow
/// github-readme-activity-graph (`src/createChart.ts`, `styles/graphStyle.ts`).
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
    let count = days.len();

    // Drawing area: (x1, y1) is the bottom left, (x2, y2) the top right.
    let x1 = PAD_LEFT + Y_OFFSET;
    let x2 = (WIDTH - PAD_RIGHT).max(x1 + 1.0);
    let y2 = PAD_TOP;
    let y1 = (height - PAD_BOTTOM - X_OFFSET).max(y2 + 1.0);
    let x_length = x2 - x1;
    let y_length = y1 - y2;

    let step_length = x_length / (count.max(2) - 1) as f64;
    let x_at = |index: usize| x1 + step_length * index as f64;

    let high = days.iter().map(|(_, value)| *value).max().unwrap_or(0);
    let bounds = bounds(y_length, f64::from(high), 0.0, SCALE_MIN_SPACE);
    let y_at = |value: f64| y1 - y_length * (value - bounds.min) / bounds.range;

    let mut body = String::with_capacity(16_000);

    // Title: centered on the card.
    let mut title_half_width = 0.0;
    if !graph.hide_title {
        let title = match &graph.title {
            Some(title) => ellipsize(title, TITLE_LIMIT),
            None => format!(
                "{}'s Contribution Graph",
                calendar.name.as_deref().unwrap_or(&calendar.login)
            ),
        };
        title_half_width = title.chars().count() as f64 * TITLE_CHAR_WIDTH / 2.0;
        text(
            &mut body,
            Anchor::Middle,
            (WIDTH / 2.0, 42.0),
            "sans header",
            &colors.title,
            &title,
        );
    }

    // Summary row: the total for the plotted days and their date range. It
    // sits at the right of the title row, or under a title too long to share it.
    if !graph.hide_summary {
        let summary = summary_text(&days);
        let width = summary.chars().count() as f64 * SUMMARY_CHAR_WIDTH;
        let fits = WIDTH / 2.0 + title_half_width + 24.0 < x2 - width;
        let (anchor, position) = if fits || graph.hide_title {
            (Anchor::End, (x2, 42.0))
        } else {
            (Anchor::Middle, (WIDTH / 2.0, 64.0))
        };
        text(
            &mut body,
            anchor,
            position,
            "sans summary",
            &colors.text,
            &summary,
        );
    }

    // Grid lines: one for every day, one for every value tick.
    let mut grid = String::new();
    let mut labels = String::new();
    for (index, (date, _)) in days.iter().enumerate() {
        let x = x_at(index);
        if graph.grid {
            let _ = write!(
                grid,
                r#"<line x1="{x:.2}" x2="{x:.2}" y1="{y2:.2}" y2="{y1:.2}" class="ct-grid" stroke="{}"/>"#,
                colors.text
            );
        }
        let date = *date;
        match graph.labels {
            LabelMode::None => {}
            LabelMode::Auto | LabelMode::Number => text(
                &mut labels,
                Anchor::Start,
                (x - 4.5, y1 + 20.0),
                "sans label",
                &colors.text,
                &date.day().to_string(),
            ),
            LabelMode::Date => text(
                &mut labels,
                Anchor::Start,
                (x - 4.5, y1 + 20.0),
                "sans label",
                &colors.text,
                &short_date(date, false),
            ),
            LabelMode::Sideways => sideways_text(
                &mut labels,
                (x + 4.0, y1 + 8.0),
                &colors.text,
                &short_date(date, false),
            ),
        }
    }
    for value in &bounds.values {
        let y = y_at(*value);
        if graph.grid {
            let _ = write!(
                grid,
                r#"<line y1="{y:.2}" y2="{y:.2}" x1="{x1:.2}" x2="{x2:.2}" class="ct-grid" stroke="{}"/>"#,
                colors.text
            );
        }
        text(
            &mut labels,
            Anchor::End,
            (x1 - 10.0, y + 4.5),
            "sans label",
            &colors.text,
            &number(*value),
        );
    }
    let _ = write!(body, r#"<g>{grid}</g>"#);

    // Area, line and points.
    let points = days
        .iter()
        .enumerate()
        .map(|(index, (_, value))| (x_at(index), y_at(f64::from(*value))))
        .collect::<Vec<_>>();
    let path = if graph.smooth {
        smooth_path(&points)
    } else {
        straight_path(&points)
    };
    let mut series = String::new();
    if graph.area && path.len() > 1 {
        let base = y_at(0.0_f64.min(bounds.max).max(bounds.min));
        let (first, last) = (points[0], points[count - 1]);
        let mut area = vec![
            Segment::Move(first.0, base),
            Segment::Line(first.0, first.1),
        ];
        area.extend(path[1..].iter().copied());
        area.push(Segment::Line(last.0, base));
        let _ = write!(
            series,
            r#"<path d="{}" class="ct-area" fill="{}"/>"#,
            stringify(&area, true),
            colors.area
        );
    }
    if !path.is_empty() {
        let _ = write!(
            series,
            r#"<path d="{}" class="ct-line" fill="none" stroke="{}"/>"#,
            stringify(&path, false),
            colors.line
        );
    }
    for ((date, value), (x, y)) in days.iter().zip(&points) {
        let _ = write!(
            series,
            r#"<line x1="{x:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" class="ct-point" stroke="{}"><title>{}: {value}</title></line>"#,
            x + 0.01,
            colors.point,
            short_date(*date, false),
        );
    }
    let _ = write!(body, "<g>{series}</g><g>{labels}</g>");

    // Axis titles.
    if !graph.hide_axis_titles {
        let node = Text::new("Days")
            .set("x", format!("{:.2}", PAD_LEFT + Y_OFFSET + x_length / 2.0))
            .set("y", format!("{:.2}", PAD_TOP + y_length + X_OFFSET))
            .set("class", "sans label")
            .set("fill", colors.text.as_str())
            .set("dominant-baseline", "text-after-edge")
            .set("text-anchor", "middle");
        body.push_str(&node.to_string());
        let (tx, ty) = (PAD_LEFT, PAD_TOP + y_length / 2.0);
        let node = Text::new("Contributions")
            .set("x", format!("{tx:.2}"))
            .set("y", format!("{ty:.2}"))
            .set("class", "sans label")
            .set("fill", colors.text.as_str())
            .set("dominant-baseline", "hanging")
            .set("text-anchor", "middle")
            .set("transform", format!("rotate(-90, {tx:.2}, {ty:.2})"));
        body.push_str(&node.to_string());
    }

    // The line is dashed with a gap longer than itself and the dashes slide
    // in (the reference uses 5000, which a very long line would exceed).
    let length = points
        .windows(2)
        .map(|pair| (pair[1].0 - pair[0].0).hypot(pair[1].1 - pair[0].1))
        .sum::<f64>();
    let dash = (length * 1.5).ceil().max(5_000.0) as u64;
    let mut css = style(card.animate);
    css.push_str(GRAPH_STYLE);
    if card.animate {
        let _ = write!(
            css,
            ".ct-line{{stroke-dasharray:{dash};animation:dash 5s ease-in-out both}}@keyframes dash{{from{{stroke-dashoffset:{dash}}}to{{stroke-dashoffset:0}}}}{GRAPH_POINT_ANIMATION}"
        );
    }

    // The card is drawn at its natural 1200 units for the medium size.
    let (medium_width, _) = CardSize::Medium.dimensions();
    let (size_width, _) = card.size.dimensions();
    let pixel_width = (f64::from(size_width) * WIDTH / f64::from(medium_width)).round() as u32;
    let pixel_height = (f64::from(pixel_width) * height / WIDTH).round() as u32;
    Ok(Document::new()
        .set("width", pixel_width)
        .set("height", pixel_height)
        .set("viewBox", (0, 0, 1200, graph.height))
        .set("fill", "none")
        .add(color_comment(&card))
        .add(Style::new(css))
        .add(
            Rectangle::new()
                .set("x", 0.5)
                .set("y", 0.5)
                .set("width", WIDTH - 1.0)
                .set("height", height - 1.0)
                .set("rx", graph.radius)
                .set("fill", colors.background)
                .set("stroke", colors.border),
        )
        .add(Blob::new(body))
        .to_string())
}

/// Text sizes and the grid, line, area and point styles of the reference
/// (`graphStyle.ts`). Labels are 12px, 600 weight.
const GRAPH_STYLE: &str = ".header{font-size:20px;font-weight:600}.summary{font-size:14px;font-weight:400}.label{font-size:12px;font-weight:600}.ct-grid{stroke-width:1px;stroke-opacity:.3;stroke-dasharray:2px}.ct-line{stroke-width:4px;stroke-linecap:butt}.ct-area{stroke:none;fill-opacity:.1}.ct-point{stroke-width:10px;stroke-linecap:round}";

/// Points slide in from the left and fade up over one second, all together.
const GRAPH_POINT_ANIMATION: &str = ".ct-point{animation:blink 1s ease-in-out both}@keyframes blink{from{opacity:0;transform:translateX(-20px)}to{opacity:1;transform:translateX(0)}}";

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

/// A whole number without a trailing `.0`.
fn number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// Value axis: where it starts and ends, and where its ticks are.
struct Bounds {
    min: f64,
    max: f64,
    range: f64,
    values: Vec<f64>,
}

/// Smallest factor of `num` (Pollard's rho), as the reference computes it to
/// find a whole-number tick step.
fn smallest_factor(num: u64) -> u64 {
    if num <= 1 {
        return 1;
    }
    if num.is_multiple_of(2) {
        return 2;
    }
    fn gcd(p: u128, q: u128) -> u128 {
        if q == 0 { p } else { gcd(q, p % q) }
    }
    let n = u128::from(num);
    let f = |x: u128| x * x + 1;
    let (mut x1, mut x2) = (2_u128, 2_u128);
    loop {
        x1 = f(x1) % n;
        x2 = f(f(x2)) % n;
        let divisor = gcd(x1.abs_diff(x2), n);
        if divisor != 1 {
            return divisor as u64;
        }
    }
}

/// The value axis of the reference: Chartist 0.11's `getBounds` with
/// `onlyInteger: true`, `low: 0` and `scaleMinSpace: 20`.
fn bounds(axis_length: f64, mut high: f64, mut low: f64, scale_min_space: f64) -> Bounds {
    if high <= low {
        if low == 0.0 {
            high = 1.0;
        } else if low < 0.0 {
            high = 0.0;
        } else if high > 0.0 {
            low = 0.0;
        } else {
            high = 1.0;
            low = 0.0;
        }
    }
    let project = |length: f64, range: f64| length / range * axis_length;

    let oom = ((high - low).abs().ln() / std::f64::consts::LN_10).floor();
    let mut step = 10_f64.powf(oom);
    let mut min = (low / step).floor() * step;
    let mut max = (high / step).ceil() * step;
    let range = max - min;

    let scale_up = project(step, range) < scale_min_space;
    let factor = smallest_factor(range as u64) as f64;

    if project(1.0, range) >= scale_min_space {
        step = 1.0;
    } else if factor < step && project(factor, range) >= scale_min_space {
        step = factor;
    } else {
        for _ in 0..1000 {
            if scale_up && project(step, range) <= scale_min_space {
                step *= 2.0;
            } else if !scale_up && project(step / 2.0, range) >= scale_min_space {
                step /= 2.0;
                if step % 1.0 != 0.0 {
                    step *= 2.0;
                    break;
                }
            } else {
                break;
            }
        }
    }

    const EPSILON: f64 = 2.221e-16;
    let step = step.max(EPSILON);
    let safe_increment = |mut value: f64, increment: f64| {
        let before = value;
        value += increment;
        if before == value {
            value *= 1.0 + if increment > 0.0 { EPSILON } else { -EPSILON };
        }
        value
    };
    while min + step <= low {
        min = safe_increment(min, step);
    }
    while max - step >= high {
        max = safe_increment(max, -step);
    }

    let mut values: Vec<f64> = Vec::new();
    let mut tick = min;
    while tick <= max {
        let value = (tick * 1e8).round() / 1e8;
        if values.last() != Some(&value) {
            values.push(value);
        }
        tick = safe_increment(tick, step);
    }
    Bounds {
        min,
        max,
        range: max - min,
        values,
    }
}

/// `520 contributions · Aug 23 – Nov 20`: the total of the plotted days and
/// their first and last date (with the year when the range spans years).
fn summary_text(days: &[(NaiveDate, u32)]) -> String {
    let total = days.iter().map(|(_, count)| u64::from(*count)).sum::<u64>();
    let digits = total.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    let noun = if total == 1 {
        "contribution"
    } else {
        "contributions"
    };
    let (Some((first, _)), Some((last, _))) = (days.first(), days.last()) else {
        return format!("{grouped} {noun}");
    };
    let years = first.year() != last.year();
    if first == last {
        return format!("{grouped} {noun} · {}", short_date(*first, years));
    }
    format!(
        "{grouped} {noun} · {} – {}",
        short_date(*first, years),
        short_date(*last, years)
    )
}

/// `Nov 3`, or `Nov 3 '25` when the range spans years.
fn short_date(date: NaiveDate, with_year: bool) -> String {
    if with_year {
        date.format("%b %-d '%y").to_string()
    } else {
        date.format("%b %-d").to_string()
    }
}

/// One piece of a drawn path.
#[derive(Clone, Copy)]
enum Segment {
    Move(f64, f64),
    Line(f64, f64),
    Curve(f64, f64, f64, f64, f64, f64),
}

fn stringify(path: &[Segment], close: bool) -> String {
    let mut out = String::new();
    for segment in path {
        let _ = match *segment {
            Segment::Move(x, y) => write!(out, "M{x:.2},{y:.2}"),
            Segment::Line(x, y) => write!(out, "L{x:.2},{y:.2}"),
            Segment::Curve(a, b, c, d, x, y) => {
                write!(out, "C{a:.2},{b:.2},{c:.2},{d:.2},{x:.2},{y:.2}")
            }
        };
    }
    if close {
        out.push('Z');
    }
    out
}

fn straight_path(points: &[(f64, f64)]) -> Vec<Segment> {
    points
        .iter()
        .enumerate()
        .map(|(index, (x, y))| {
            if index == 0 {
                Segment::Move(*x, *y)
            } else {
                Segment::Line(*x, *y)
            }
        })
        .collect()
}

/// Monotone cubic curve through the points (Fritsch-Carlson), as Chartist's
/// `monotoneCubic` does. Fewer than three points stay straight.
fn smooth_path(points: &[(f64, f64)]) -> Vec<Segment> {
    let n = points.len();
    if n <= 2 {
        return straight_path(points);
    }
    let dx: Vec<f64> = (0..n - 1).map(|i| points[i + 1].0 - points[i].0).collect();
    let dy: Vec<f64> = (0..n - 1).map(|i| points[i + 1].1 - points[i].1).collect();
    let d: Vec<f64> = (0..n - 1).map(|i| dy[i] / dx[i]).collect();
    let mut m = vec![0.0; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        if d[i] == 0.0 || d[i - 1] == 0.0 || (d[i - 1] > 0.0) != (d[i] > 0.0) {
            m[i] = 0.0;
        } else {
            let value = 3.0 * (dx[i - 1] + dx[i])
                / ((2.0 * dx[i] + dx[i - 1]) / d[i - 1] + (dx[i] + 2.0 * dx[i - 1]) / d[i]);
            m[i] = if value.is_finite() { value } else { 0.0 };
        }
    }
    let mut path = vec![Segment::Move(points[0].0, points[0].1)];
    for i in 0..n - 1 {
        path.push(Segment::Curve(
            points[i].0 + dx[i] / 3.0,
            points[i].1 + m[i] * dx[i] / 3.0,
            points[i + 1].0 - dx[i] / 3.0,
            points[i + 1].1 - m[i + 1] * dx[i] / 3.0,
            points[i + 1].0,
            points[i + 1].1,
        ));
    }
    path
}

/// A date written bottom to top, hanging below the axis with its end at `(x, y)`.
fn sideways_text(svg: &mut String, (x, y): (f64, f64), fill: &str, content: &str) {
    let node = Text::new(content)
        .set("x", format!("{x:.2}"))
        .set("y", format!("{y:.2}"))
        .set("class", "sans micro")
        .set("fill", fill)
        .set("text-anchor", "end")
        .set("transform", format!("rotate(-90 {x:.2} {y:.2})"));
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

    fn ticks(max: u32, axis_length: f64) -> Vec<f64> {
        bounds(axis_length, f64::from(max), 0.0, SCALE_MIN_SPACE).values
    }

    #[test]
    fn value_axis_matches_the_reference() {
        // Height 420 gives a 270 unit plot. A peak of 20 is ticked every 2,
        // as in github-readme-activity-graph.
        let twenty = ticks(20, 270.0);
        assert_eq!(twenty.first(), Some(&0.0));
        assert_eq!(twenty.last(), Some(&20.0));
        assert_eq!(twenty.len(), 11);
        // Nothing to plot still gets an axis.
        assert!(ticks(0, 270.0).len() >= 2);
        // Ticks are whole numbers and cover the data.
        for max in [1, 3, 7, 13, 31, 47, 99, 150, 1_234] {
            let values = ticks(max, 270.0);
            assert!(values.iter().all(|value| value.fract() == 0.0), "{max}");
            assert!(*values.last().unwrap() >= f64::from(max), "{max}");
        }
    }

    #[test]
    fn smallest_factor_follows_the_reference() {
        assert_eq!(smallest_factor(1), 1);
        assert_eq!(smallest_factor(20), 2);
        assert_eq!(smallest_factor(9), 3);
        assert_eq!(smallest_factor(13), 13);
    }
}
