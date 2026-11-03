use std::fmt::Write as _;

use svg::{
    Document,
    node::{
        Blob,
        element::{Rectangle, Style, Text},
    },
};
use thiserror::Error;

use super::{CardOptions, CardTheme, PeriodDays};
use crate::model::{ActivitySnapshot, ContributorActivity, DailyActivity};

pub trait CardRenderer {
    fn render(
        &self,
        snapshot: &ActivitySnapshot,
        options: CardOptions,
    ) -> Result<String, CardRenderError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SvgCardRenderer;

impl CardRenderer for SvgCardRenderer {
    fn render(
        &self,
        snapshot: &ActivitySnapshot,
        options: CardOptions,
    ) -> Result<String, CardRenderError> {
        render(snapshot, options)
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[error("snapshot does not contain enough history for a {0}-day card")]
pub struct CardRenderError(usize);

pub fn render(
    snapshot: &ActivitySnapshot,
    options: CardOptions,
) -> Result<String, CardRenderError> {
    let period = snapshot
        .period(options.period.get())
        .ok_or(CardRenderError(options.period.get()))?;
    let issue_current = ActivitySnapshot::issue_ratio(period.current);
    let issue_previous = ActivitySnapshot::issue_ratio(period.previous);
    let pulls_current = f64::from(ActivitySnapshot::pulls_opened(period.current));
    let pulls_previous = f64::from(ActivitySnapshot::pulls_opened(period.previous));
    let pushes_current = f64::from(ActivitySnapshot::pushes(period.current));
    let pushes_previous = f64::from(ActivitySnapshot::pushes(period.previous));
    let palette = Palette::for_theme(options.theme);
    let mut body = String::with_capacity(20_000);

    render_header(&mut body, period.current, options.period, palette);
    render_metric(
        &mut body,
        0,
        MetricStyle::new("#406CC4", "Opened/Closed Issue Ratio"),
        issue_current,
        issue_previous,
        options.period.comparison_label(),
        palette,
    );
    render_metric(
        &mut body,
        274,
        MetricStyle::new("#7B58C9", "Pull Requests Opened"),
        pulls_current,
        pulls_previous,
        options.period.comparison_label(),
        palette,
    );
    render_metric(
        &mut body,
        548,
        MetricStyle::new("#BA473E", "Pushes"),
        pushes_current,
        pushes_previous,
        options.period.comparison_label(),
        palette,
    );
    render_dual_chart(
        &mut body,
        0,
        DualChartStyle::new("Issues", "#406CC4", "#61adf4"),
        period.current,
        |day| (day.issues_opened, day.issues_closed),
        palette,
    );
    render_dual_chart(
        &mut body,
        274,
        DualChartStyle::new("Pull Requests", "#7B58C9", "#cf54fc"),
        period.current,
        |day| (day.pulls_opened, day.pulls_closed),
        palette,
    );
    render_push_chart(&mut body, period.current, palette);
    render_contributors(&mut body, &snapshot.contributors, options.period, palette);

    let (width, height) = options.size.dimensions();
    Ok(Document::new()
        .set("width", width)
        .set("height", height)
        .set("viewBox", (0, 0, 814, 318))
        .set("fill", "none")
        .add(Style::new(
            r#".sans{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Helvetica,Arial,sans-serif,"Apple Color Emoji","Segoe UI Emoji"}.mono{font-family:ui-monospace,SFMono-Regular,"SF Mono",Menlo,Consolas,"Liberation Mono",monospace;font-size:12px;letter-spacing:-.5px}.bold{font-weight:500}.small{font-size:14px}.smaller{font-size:12px}.glow{paint-order:stroke}"#,
        ))
        .add(
            Rectangle::new()
                .set("x", 0.5)
                .set("y", 0.5)
                .set("width", 813)
                .set("height", 317)
                .set("fill", palette.background),
        )
        .add(Blob::new(body))
        .to_string())
}

#[derive(Clone, Copy)]
struct Palette {
    background: &'static str,
    panel_fill: &'static str,
    panel_stroke: &'static str,
    panel_highlight: &'static str,
    secondary_text: &'static str,
    trend_up: &'static str,
    trend_down: &'static str,
    contributor: &'static str,
}

impl Palette {
    fn for_theme(theme: CardTheme) -> Self {
        match theme {
            CardTheme::Light => Self {
                background: "transparent",
                panel_fill: "none",
                panel_stroke: "rgba(0,0,0,0.15)",
                panel_highlight: "rgba(255,255,255,0.3)",
                secondary_text: "#777",
                trend_up: "#459631",
                trend_down: "#CF3E3E",
                contributor: "#487B41",
            },
            CardTheme::Dark => Self {
                background: "#0d1117",
                panel_fill: "#161b22",
                panel_stroke: "#30363d",
                panel_highlight: "rgba(255,255,255,0.08)",
                secondary_text: "#8b949e",
                trend_up: "#56d364",
                trend_down: "#f85149",
                contributor: "#7ee787",
            },
        }
    }
}

fn render_header(svg: &mut String, days: &[DailyActivity], period: PeriodDays, palette: Palette) {
    panel(svg, 0, 0, 813, 47, palette);
    svg.push_str(
        r##"<g transform="translate(10 16)" fill="#ec4899"><rect width="20" height="2" rx="1"/><rect y="14" width="20" height="2" rx="1"/><rect y="4" width="6" height="2" rx="1"/><rect x="7" y="4" width="6" height="2" rx="1"/><rect x="14" y="4" width="6" height="2" rx="1"/><rect y="8" width="6" height="4" rx="1"/><rect x="7" y="8" width="6" height="4" rx="1"/><rect x="14" y="8" width="6" height="4" rx="1"/></g>"##,
    );
    write!(
        svg,
        r##"<text x="38" y="29" class="sans small bold glow" fill="#ec4899" stroke-width="0.5">{} Contributions in the Last {} Days</text>"##,
        days.iter().map(DailyActivity::contributions).sum::<u32>(),
        period.get(),
    )
    .unwrap();
    let activity = days
        .iter()
        .map(DailyActivity::contributions)
        .collect::<Vec<_>>();
    let max = activity.iter().copied().max().unwrap_or(0);
    let slot = 270.0 / activity.len() as f64;
    let width = slot * 8.0 / 9.0;
    for (index, value) in activity.iter().enumerate() {
        write!(
            svg,
            r##"<rect data-header-day="{}" x="{:.1}" y="20" width="{:.1}" height="8" rx="1.5" opacity="{:.1}" fill="#ec4899"/>"##,
            index + 1,
            533.5 + index as f64 * slot,
            width,
            opacity(*value, max)
        )
        .unwrap();
    }
}

#[derive(Clone, Copy)]
struct MetricStyle<'a> {
    color: &'a str,
    label: &'a str,
}

impl<'a> MetricStyle<'a> {
    fn new(color: &'a str, label: &'a str) -> Self {
        Self { color, label }
    }
}

fn render_metric(
    svg: &mut String,
    x: u32,
    style: MetricStyle<'_>,
    current: f64,
    previous: f64,
    comparison_label: &str,
    palette: Palette,
) {
    panel(svg, x, 58, 264, 70, palette);
    let delta = current - previous;
    let trend_color = if delta > 0.0 {
        palette.trend_up
    } else {
        palette.trend_down
    };
    let arrow = if delta > 0.0 {
        "M174 55V47m0 0-4 4m4-4 4 4"
    } else {
        "M174 47v8m0 0-4-4m4 4 4-4"
    };
    write!(
        svg,
        r#"<text x="{}" y="82" class="sans small bold" fill="{}">{} {}</text>"#,
        x + 10,
        style.color,
        format_value(current),
        style.label
    )
    .unwrap();
    write!(
        svg,
        r#"<text x="{}" y="114" class="mono smaller bold" fill="{}">{}</text><path d="{}" transform="translate({} 58)" stroke="{}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><text x="{}" y="114" text-anchor="end" class="mono smaller bold" fill="{}">{}</text>"#,
        x + 10,
        palette.secondary_text,
        format_delta(current, previous),
        arrow,
        x,
        trend_color,
        x + 254,
        trend_color,
        comparison_label,
    )
    .unwrap();
}

#[derive(Clone, Copy)]
struct DualChartStyle<'a> {
    title: &'a str,
    closed_color: &'a str,
    opened_color: &'a str,
}

impl<'a> DualChartStyle<'a> {
    fn new(title: &'a str, closed_color: &'a str, opened_color: &'a str) -> Self {
        Self {
            title,
            closed_color,
            opened_color,
        }
    }
}

fn render_dual_chart<F>(
    svg: &mut String,
    x: u32,
    style: DualChartStyle<'_>,
    days: &[DailyActivity],
    values: F,
    palette: Palette,
) where
    F: Fn(&DailyActivity) -> (u32, u32),
{
    panel(svg, x, 138, 264, 120, palette);
    if x == 0 {
        svg.push_str(
            r##"<g transform="translate(10 158)"><path d="M8 9.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z" fill="url(#issue-dot)"/><path fill-rule="evenodd" clip-rule="evenodd" d="M8 0a8 8 0 1 0 0 16A8 8 0 0 0 8 0ZM1.5 8a6.5 6.5 0 1 1 13 0 6.5 6.5 0 0 1-13 0Z" fill="url(#issue-ring)"/><defs><linearGradient id="issue-dot" x1="8" y1="6.5" x2="8" y2="9.5" gradientUnits="userSpaceOnUse"><stop stop-color="#00BCF7"/><stop offset=".729" stop-color="#326CE5"/></linearGradient><linearGradient id="issue-ring" x1="8" y1="0" x2="8" y2="16" gradientUnits="userSpaceOnUse"><stop stop-color="#00BCF7"/><stop offset=".729" stop-color="#326CE5"/></linearGradient></defs></g>"##,
        );
    } else {
        svg.push_str(
            r##"<g transform="translate(284 158)"><path fill-rule="evenodd" clip-rule="evenodd" d="M6.176 3.073 8.572.677A.25.25 0 0 1 9 .854v4.792a.25.25 0 0 1-.427.177L6.176 3.427a.25.25 0 0 1 0-.354ZM2.749 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm-2.25.75a2.25 2.25 0 1 1 3 2.122v5.256a2.25 2.25 0 1 1-1.5 0V5.372a2.25 2.25 0 0 1-1.5-2.122ZM10 2.5H9V4h1a1 1 0 0 1 1 1v5.628a2.25 2.25 0 1 0 1.5 0V5A2.5 2.5 0 0 0 10 2.5Zm1 10.25a.75.75 0 1 1 1.5 0 .75.75 0 0 1-1.5 0ZM2.75 12a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Z" fill="url(#pull-icon)"/><defs><linearGradient id="pull-icon" x1="0" y1="0" x2="0" y2="16" gradientUnits="userSpaceOnUse"><stop stop-color="#CE5AFF"/><stop offset="1" stop-color="#7C64BD"/></linearGradient></defs></g>"##,
        );
    }
    write!(
        svg,
        r#"<text x="{}" y="171" class="sans small bold" fill="{}">{}</text><circle cx="{}" cy="158" r="5" fill="{}"/><text x="{}" y="162" class="sans smaller" fill="{}">Opened</text><circle cx="{}" cy="173" r="5" fill="{}"/><text x="{}" y="177" class="sans smaller" fill="{}">Closed</text>"#,
        x + 34,
        style.closed_color,
        style.title,
        x + 195,
        style.opened_color,
        x + 205,
        style.opened_color,
        x + 195,
        style.closed_color,
        x + 205,
        style.closed_color,
    )
    .unwrap();
    let max = days
        .iter()
        .flat_map(|day| {
            let (opened, closed) = values(day);
            [opened, closed]
        })
        .max()
        .unwrap_or(0);
    for (index, day) in days.iter().enumerate() {
        let (opened, closed) = values(day);
        chart_bar(
            svg,
            x,
            days.len(),
            index,
            opened,
            max,
            BarStyle::new(style.opened_color, 0.88, 0.0),
        );
        chart_bar(
            svg,
            x,
            days.len(),
            index,
            closed,
            max,
            BarStyle::new(style.closed_color, 0.4, 0.48),
        );
    }
}

fn render_push_chart(svg: &mut String, days: &[DailyActivity], palette: Palette) {
    let x = 548;
    panel(svg, x, 138, 264, 120, palette);
    svg.push_str(
        r##"<g transform="translate(558 158)"><path fill-rule="evenodd" clip-rule="evenodd" d="M1 2.5A2.5 2.5 0 0 1 3.5 0h8.75a.75.75 0 0 1 .75.75v3.5a.75.75 0 0 1-1.5 0V1.5h-8a1 1 0 0 0-1 1v6.708A2.492 2.492 0 0 1 3.5 9h3.25a.75.75 0 0 1 0 1.5H3.5a1 1 0 1 0 0 2h5.75a.75.75 0 0 1 0 1.5H3.5A2.5 2.5 0 0 1 1 11.5v-9zm13.23 7.79a.75.75 0 0 0 1.06-1.06l-2.505-2.505a.75.75 0 0 0-1.06 0L9.22 9.229a.75.75 0 0 0 1.06 1.061l1.225-1.224v6.184a.75.75 0 0 0 1.5 0V9.066l1.224 1.224z" fill="url(#push-icon)"/><defs><linearGradient id="push-icon" x1="0" y1="0" x2="0" y2="16" gradientUnits="userSpaceOnUse"><stop stop-color="#ff553a"/><stop offset="1" stop-color="#cd2e31"/></linearGradient></defs></g><text x="582" y="171" class="sans small bold" fill="#BA473E">Pushes</text><circle cx="743" cy="158" r="5" fill="#ff7a35"/><text x="753" y="162" class="sans smaller" fill="#ff7a35">Pushes</text>"##,
    );
    let max = days.iter().map(|day| day.pushes).max().unwrap_or(0);
    for (index, day) in days.iter().enumerate() {
        chart_bar(
            svg,
            x,
            days.len(),
            index,
            day.pushes,
            max,
            BarStyle::new("#ff7a35", 0.88, 0.0),
        );
    }
}

#[derive(Clone, Copy)]
struct BarStyle<'a> {
    color: &'a str,
    width: f64,
    offset: f64,
}

impl<'a> BarStyle<'a> {
    fn new(color: &'a str, width: f64, offset: f64) -> Self {
        Self {
            color,
            width,
            offset,
        }
    }
}

fn chart_bar(
    svg: &mut String,
    panel_x: u32,
    days: usize,
    index: usize,
    value: u32,
    max: u32,
    style: BarStyle<'_>,
) {
    let height = if max == 0 {
        0.0
    } else {
        f64::from(value) / f64::from(max) * 50.0
    };
    let slot = 262.65 / days as f64;
    let rectangle = Rectangle::new()
        .set(
            "x",
            f64::from(panel_x) + 1.0 + index as f64 * slot + style.offset * slot,
        )
        .set("y", 257.5 - height)
        .set("width", style.width * slot)
        .set("height", height)
        .set("fill", style.color);
    svg.push_str(&rectangle.to_string());
}

fn render_contributors(
    svg: &mut String,
    contributors: &[ContributorActivity],
    period: PeriodDays,
    palette: Palette,
) {
    panel(svg, 0, 270, 813, 47, palette);
    svg.push_str(
        r##"<g transform="translate(10 286)"><path d="M9.795 12.972c-.326.205-.6.364-.794.474a22.2 22.2 0 0 1-3.278-2.281C3.883 9.598 2.241 7.562 2.241 5.374c0-1.833 1.502-3.133 2.931-3.133 1.453 0 2.638.956 3.169 2.547a.695.695 0 0 0 1.318 0c.531-1.591 1.715-2.547 3.169-2.547 1.429 0 2.931 1.3 2.931 3.133 0 2.188-1.642 4.224-3.482 5.791-.899.766-1.802 1.382-2.482 1.807ZM9.345 14.843a25.4 25.4 0 0 0 3.833-2.619c1.887-1.606 3.972-4.002 3.972-6.85C17.15 2.773 15.025.85 12.828.85A4.41 4.41 0 0 0 9 2.976 4.412 4.412 0 0 0 5.172.85C2.975.85.85 2.774.85 5.374c0 2.848 2.084 5.244 3.972 6.85a25.4 25.4 0 0 0 3.728 2.562.695.695 0 0 0 .45.148.696.696 0 0 0 .345-.091Z" fill="url(#heart-icon)"/><defs><linearGradient id="heart-icon" x1="9" y1="1" x2="9" y2="14.784" gradientUnits="userSpaceOnUse"><stop stop-color="#36C5F0"/><stop offset=".729" stop-color="#55BA3C"/></linearGradient></defs></g>"##,
    );
    svg.push_str(
        &Text::new("Top Contributors")
            .set("x", 36)
            .set("y", 299)
            .set("class", "sans small bold")
            .set("fill", palette.contributor)
            .to_string(),
    );
    let mut ranked = contributors
        .iter()
        .filter_map(|contributor| {
            let start = contributor.days.len().checked_sub(period.get())?;
            let days = &contributor.days[start..];
            Some(RankedContributor {
                contributor,
                total: days.iter().sum(),
                buckets: activity_buckets(days),
            })
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .total
            .cmp(&left.total)
            .then_with(|| left.contributor.name.cmp(&right.contributor.name))
    });
    ranked.truncate(5);
    let global_max = ranked
        .iter()
        .flat_map(|contributor| contributor.buckets.iter())
        .copied()
        .max()
        .unwrap_or(0);
    let bucket_count = period.get().min(30);
    for index in 0..5 {
        let x = 190 + index * 130;
        let contributor = ranked.get(index);
        let name = contributor
            .map(|item| truncate_name(&item.contributor.name))
            .unwrap_or_default();
        let text = Text::new(name)
            .set("x", x)
            .set("y", 289)
            .set("class", "mono bold smaller")
            .set("fill", palette.contributor);
        svg.push_str(&text.to_string());
        for bucket in 0..bucket_count {
            let value = contributor
                .and_then(|item| item.buckets.get(bucket))
                .copied()
                .unwrap_or(0);
            let cell_x = x + (bucket % 15) * 6;
            let cell_y = 298 + (bucket / 15) * 6;
            let rectangle = Rectangle::new()
                .set("x", cell_x)
                .set("y", cell_y)
                .set("width", 5)
                .set("height", 5)
                .set("opacity", opacity(value, global_max))
                .set("fill", palette.contributor);
            svg.push_str(&rectangle.to_string());
        }
    }
}

struct RankedContributor<'a> {
    contributor: &'a ContributorActivity,
    total: u32,
    buckets: Vec<u32>,
}

fn activity_buckets(days: &[u32]) -> Vec<u32> {
    let bucket_count = days.len().min(30);
    (0..bucket_count)
        .map(|bucket| {
            let start = bucket * days.len() / bucket_count;
            let end = (bucket + 1) * days.len() / bucket_count;
            days[start..end].iter().sum()
        })
        .collect()
}

fn panel(svg: &mut String, x: u32, y: u32, width: u32, height: u32, palette: Palette) {
    let panel = Rectangle::new()
        .set("x", f64::from(x) + 0.5)
        .set("y", f64::from(y) + 0.5)
        .set("width", width)
        .set("height", height)
        .set("rx", 4.5)
        .set("fill", palette.panel_fill)
        .set("stroke", palette.panel_stroke);
    let highlight = Rectangle::new()
        .set("x", f64::from(x) + 0.5)
        .set("y", f64::from(y) + 0.5)
        .set("width", width)
        .set("height", height)
        .set("rx", 4.5)
        .set("fill", "none")
        .set("stroke", palette.panel_highlight);
    svg.push_str(&panel.to_string());
    svg.push_str(&highlight.to_string());
}

fn opacity(value: u32, max: u32) -> f64 {
    if max == 0 || value == 0 {
        0.2
    } else {
        (0.2 + f64::from(value) / f64::from(max) * 0.8).clamp(0.2, 1.0)
    }
}

fn format_value(value: f64) -> String {
    trim_decimal(value, 2)
}

fn format_delta(current: f64, previous: f64) -> String {
    let delta = current - previous;
    let percent = if previous == 0.0 {
        if current == 0.0 { 0.0 } else { 100.0 }
    } else {
        delta / previous * 100.0
    };
    let sign = if delta > 0.0 { "+" } else { "" };
    format!(
        "{sign}{} ({}%)",
        trim_decimal(delta, 2),
        trim_decimal(percent, 2)
    )
}

fn trim_decimal(value: f64, precision: usize) -> String {
    let mut formatted = format!("{value:.precision$}");
    if formatted.contains('.') {
        while formatted.ends_with('0') {
            formatted.pop();
        }
        if formatted.ends_with('.') {
            formatted.pop();
        }
    }
    formatted
}

fn truncate_name(name: &str) -> String {
    let mut chars = name.chars();
    let short = chars.by_ref().take(16).collect::<String>();
    if chars.next().is_some() {
        format!("{}…", short.chars().take(15).collect::<String>())
    } else {
        short
    }
}
