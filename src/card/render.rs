use std::{borrow::Cow, fmt::Write as _};

use svg::{
    Document,
    node::{
        Blob,
        element::{Rectangle, Style, Text},
    },
};
use thiserror::Error;

use super::{CardOptions, CardTheme, ColorScheme, PeriodDays, Rgb};

mod extra;

use crate::model::{ActivitySnapshot, ContributorActivity, DailyActivity};
pub use extra::{render_contributors, render_languages, render_overview};

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
    let palette = &Palette::resolve(&options);
    let mut body = String::with_capacity(20_000);

    render_header(&mut body, period.current, options.period, palette);
    render_metric(
        &mut body,
        0,
        MetricStyle::new(&palette.issues.main, "Opened/Closed Issue Ratio"),
        issue_current,
        issue_previous,
        options.period.comparison_label(),
        palette,
    );
    render_metric(
        &mut body,
        274,
        MetricStyle::new(&palette.pulls.main, "Pull Requests Opened"),
        pulls_current,
        pulls_previous,
        options.period.comparison_label(),
        palette,
    );
    render_metric(
        &mut body,
        548,
        MetricStyle::new(&palette.pushes.main, "Pushes"),
        pushes_current,
        pushes_previous,
        options.period.comparison_label(),
        palette,
    );
    render_dual_chart(
        &mut body,
        0,
        4,
        DualChartStyle::new("Issues", &palette.issues),
        period.current,
        |day| (day.issues_opened, day.issues_closed),
        palette,
    );
    render_dual_chart(
        &mut body,
        274,
        5,
        DualChartStyle::new("Pull Requests", &palette.pulls),
        period.current,
        |day| (day.pulls_opened, day.pulls_closed),
        palette,
    );
    render_push_chart(&mut body, period.current, palette);
    render_contributor_strip(&mut body, &snapshot.contributors, options.period, palette);

    let (width, height) = options.size.dimensions();
    Ok(Document::new()
        .set("width", width)
        .set("height", height)
        .set("viewBox", (0, 0, 814, 318))
        .set("fill", "none")
        .add(color_comment(&options))
        .add(Style::new(style(options.animate)))
        .add(
            Rectangle::new()
                .set("x", 0.5)
                .set("y", 0.5)
                .set("width", 813)
                .set("height", 317)
                .set("fill", palette.background.to_string()),
        )
        .add(Blob::new(body))
        .to_string())
}

const BASE_STYLE: &str = r#".sans{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Helvetica,Arial,sans-serif,"Apple Color Emoji","Segoe UI Emoji"}.mono{font-family:ui-monospace,SFMono-Regular,"SF Mono",Menlo,Consolas,"Liberation Mono",monospace;font-size:12px;letter-spacing:-.5px}.bold{font-weight:500}.small{font-size:14px}.smaller{font-size:12px}.glow{paint-order:stroke}"#;

/// Entrance animations (panels rise in, bars grow, the language bar wipes in)
/// and two slow loops (a ping ring around the header icon, a bobbing trend
/// arrow). Delay classes `.dN` stagger panels, `.wN` stagger bars and cells.
/// `prefers-reduced-motion` turns it all off and shows the finished card.
fn animation_css() -> String {
    let mut css = String::from(
        ".in{animation:rise .5s ease-out both}\
         @keyframes rise{from{opacity:0;transform:translateY(8px)}}\
         .cell{animation:fade .4s ease-out both}\
         @keyframes fade{from{opacity:0}}\
         .bar{transform-box:fill-box;transform-origin:50% 100%;animation:grow .6s cubic-bezier(.2,.8,.2,1) both}\
         @keyframes grow{from{transform:scaleY(0)}}\
         .hbar,.wipe{transform-box:fill-box;transform-origin:0 50%;animation:growx .7s cubic-bezier(.2,.8,.2,1) both}\
         @keyframes growx{from{transform:scaleX(0)}}\
         .ping{opacity:0;transform-box:fill-box;transform-origin:center;animation:ping 2.8s ease-out 1s infinite}\
         @keyframes ping{0%{opacity:.7;transform:scale(1)}70%,100%{opacity:0;transform:scale(2.2)}}\
         .bob-up{animation:bob-up 2.4s ease-in-out 1.2s infinite}\
         .bob-down{animation:bob-down 2.4s ease-in-out 1.2s infinite}\
         @keyframes bob-up{0%,100%{transform:translateY(0)}50%{transform:translateY(-2px)}}\
         @keyframes bob-down{0%,100%{transform:translateY(0)}50%{transform:translateY(2px)}}",
    );
    for step in 0..=15 {
        let _ = write!(
            css,
            ".d{step}{{animation-delay:{}ms}}.w{step}{{animation-delay:{}ms}}",
            step * 70,
            350 + step * 40
        );
    }
    css.push_str("@media (prefers-reduced-motion:reduce){*{animation:none!important}}");
    css
}

/// Base styles, plus the animations when `animate` is on.
fn style(animate: bool) -> String {
    let mut css = BASE_STYLE.to_owned();
    if animate {
        css.push_str(&animation_css());
    }
    css
}

/// An XML comment naming which list position became which color.
fn color_comment(options: &CardOptions) -> svg::node::Comment {
    svg::node::Comment::new(match &options.colors {
        Some(colors) => colors.describe(),
        None => format!("repobeats theme: {:?}", options.theme),
    })
}

/// Opens a section that slides in after `step` x 70 ms. Close it with [`end_section`].
fn begin_section(svg: &mut String, step: usize) {
    let _ = write!(svg, r#"<g class="in d{}">"#, step.min(15));
}

fn end_section(svg: &mut String) {
    svg.push_str("</g>");
}

/// Delay class for item `index` of `count`, spread over the 16 wave steps.
fn wave(index: usize, count: usize) -> usize {
    (index * 16 / count.max(1)).min(15)
}

/// A data series: `main` for text and the darker bars, `light` for the lighter
/// bars, and the two stops of the icon gradient.
#[derive(Clone)]
struct Series {
    main: Cow<'static, str>,
    light: Cow<'static, str>,
    icon_from: Cow<'static, str>,
    icon_to: Cow<'static, str>,
}

impl Series {
    fn fixed(
        main: &'static str,
        light: &'static str,
        from: &'static str,
        to: &'static str,
    ) -> Self {
        Self {
            main: c(main),
            light: c(light),
            icon_from: c(from),
            icon_to: c(to),
        }
    }

    fn from_color(color: Rgb) -> Self {
        Self {
            main: color.to_string().into(),
            light: color.lighten(0.35).to_string().into(),
            icon_from: color.lighten(0.35).to_string().into(),
            icon_to: color.to_string().into(),
        }
    }
}

fn c(value: &'static str) -> Cow<'static, str> {
    Cow::Borrowed(value)
}

#[derive(Clone)]
struct Palette {
    background: Cow<'static, str>,
    panel_fill: Cow<'static, str>,
    panel_stroke: Cow<'static, str>,
    panel_highlight: Cow<'static, str>,
    secondary_text: Cow<'static, str>,
    trend_up: Cow<'static, str>,
    trend_down: Cow<'static, str>,
    contributor: Cow<'static, str>,
    text: Cow<'static, str>,
    accent: Cow<'static, str>,
    issues: Series,
    pulls: Series,
    pushes: Series,
    heart_from: Cow<'static, str>,
    heart_to: Cow<'static, str>,
    /// Stripe colors of the overview tiles, in tile order.
    stripes: [Cow<'static, str>; 8],
}

impl Palette {
    /// Colors from `options.colors` when set, otherwise from the theme.
    fn resolve(options: &CardOptions) -> Self {
        match &options.colors {
            Some(colors) => Self::custom(colors),
            None => Self::for_theme(options.theme),
        }
    }

    /// Series, icon and stripe colors that every built-in theme shares.
    fn brand() -> Self {
        Self {
            background: c("transparent"),
            panel_fill: c("none"),
            panel_stroke: c("rgba(0,0,0,0.15)"),
            panel_highlight: c("rgba(255,255,255,0.3)"),
            secondary_text: c("#777"),
            trend_up: c("#459631"),
            trend_down: c("#CF3E3E"),
            contributor: c("#487B41"),
            text: c("#24292f"),
            accent: c("#ec4899"),
            issues: Series::fixed("#406CC4", "#61adf4", "#00BCF7", "#326CE5"),
            pulls: Series::fixed("#7B58C9", "#cf54fc", "#CE5AFF", "#7C64BD"),
            pushes: Series::fixed("#BA473E", "#ff7a35", "#ff553a", "#cd2e31"),
            heart_from: c("#36C5F0"),
            heart_to: c("#55BA3C"),
            stripes: [
                c("#e3b341"),
                c("#406CC4"),
                c("#BA473E"),
                c("#7B58C9"),
                c("#459631"),
                c("#00ADD8"),
                c("#ec4899"),
                c("#ff7a35"),
            ],
        }
    }

    /// Palette from a color list. Which position does what is documented on
    /// [`ColorScheme`]. Panels, borders and muted text are mixed from
    /// background and text, so four colors are enough.
    fn custom(colors: &ColorScheme) -> Self {
        let dark = colors.is_dark();
        let text = colors.text;
        let tertiary = colors.tertiary_or_derived();
        let base = colors.background.unwrap_or(if dark {
            Rgb(13, 17, 23)
        } else {
            Rgb(255, 255, 255)
        });
        let (panel_fill, panel_stroke) = match colors.background {
            Some(background) => (
                background.mix(text, 0.06).to_string().into(),
                background.mix(text, 0.2).to_string().into(),
            ),
            None => (c("none"), text.rgba(0.2).into()),
        };
        let (trend_up, trend_down) = if dark {
            (c("#56d364"), c("#f85149"))
        } else {
            (c("#459631"), c("#CF3E3E"))
        };
        Self {
            background: colors
                .background
                .map_or(c("transparent"), |color| color.to_string().into()),
            panel_fill,
            panel_stroke,
            panel_highlight: if dark {
                c("rgba(255,255,255,0.06)")
            } else {
                c("rgba(255,255,255,0.3)")
            },
            secondary_text: base.mix(text, 0.6).to_string().into(),
            trend_up,
            trend_down,
            contributor: colors.primary.to_string().into(),
            text: text.to_string().into(),
            accent: colors.primary.to_string().into(),
            issues: Series::from_color(colors.primary),
            pulls: Series::from_color(colors.secondary),
            pushes: Series::from_color(tertiary),
            heart_from: colors.primary.to_string().into(),
            heart_to: colors.secondary.to_string().into(),
            stripes: [
                colors.primary,
                colors.secondary,
                tertiary,
                colors.primary,
                colors.secondary,
                tertiary,
                colors.primary,
                colors.secondary,
            ]
            .map(|color| color.to_string().into()),
        }
    }

    fn for_theme(theme: CardTheme) -> Self {
        match theme {
            CardTheme::Light => Self {
                background: c("transparent"),
                panel_fill: c("none"),
                panel_stroke: c("rgba(0,0,0,0.15)"),
                panel_highlight: c("rgba(255,255,255,0.3)"),
                secondary_text: c("#777"),
                trend_up: c("#459631"),
                trend_down: c("#CF3E3E"),
                contributor: c("#487B41"),
                text: c("#24292f"),
                accent: c("#ec4899"),
                ..Self::brand()
            },
            CardTheme::Dark => Self {
                background: c("#0d1117"),
                panel_fill: c("#161b22"),
                panel_stroke: c("#30363d"),
                panel_highlight: c("rgba(255,255,255,0.08)"),
                secondary_text: c("#8b949e"),
                trend_up: c("#56d364"),
                trend_down: c("#f85149"),
                contributor: c("#7ee787"),
                text: c("#e6edf3"),
                accent: c("#ec4899"),
                ..Self::brand()
            },
            CardTheme::GithubDimmed => Self {
                background: c("#22272e"),
                panel_fill: c("#2d333b"),
                panel_stroke: c("#444c56"),
                panel_highlight: c("rgba(255,255,255,0.06)"),
                secondary_text: c("#768390"),
                trend_up: c("#57ab5a"),
                trend_down: c("#e5534b"),
                contributor: c("#6bc46d"),
                text: c("#adbac7"),
                accent: c("#e275ad"),
                ..Self::brand()
            },
            CardTheme::Dracula => Self {
                background: c("#282a36"),
                panel_fill: c("#21222c"),
                panel_stroke: c("#44475a"),
                panel_highlight: c("rgba(255,255,255,0.05)"),
                secondary_text: c("#6272a4"),
                trend_up: c("#50fa7b"),
                trend_down: c("#ff5555"),
                contributor: c("#50fa7b"),
                text: c("#f8f8f2"),
                accent: c("#ff79c6"),
                ..Self::brand()
            },
            CardTheme::Nord => Self {
                background: c("#2e3440"),
                panel_fill: c("#3b4252"),
                panel_stroke: c("#4c566a"),
                panel_highlight: c("rgba(255,255,255,0.05)"),
                secondary_text: c("#9aa5b8"),
                trend_up: c("#a3be8c"),
                trend_down: c("#bf616a"),
                contributor: c("#a3be8c"),
                text: c("#eceff4"),
                accent: c("#88c0d0"),
                ..Self::brand()
            },
            CardTheme::TokyoNight => Self {
                background: c("#1a1b26"),
                panel_fill: c("#24283b"),
                panel_stroke: c("#414868"),
                panel_highlight: c("rgba(255,255,255,0.05)"),
                secondary_text: c("#7982a9"),
                trend_up: c("#9ece6a"),
                trend_down: c("#f7768e"),
                contributor: c("#9ece6a"),
                text: c("#c0caf5"),
                accent: c("#bb9af7"),
                ..Self::brand()
            },
            CardTheme::SolarizedLight => Self {
                background: c("#fdf6e3"),
                panel_fill: c("#eee8d5"),
                panel_stroke: c("#d6cfb9"),
                panel_highlight: c("rgba(255,255,255,0.4)"),
                secondary_text: c("#657b83"),
                trend_up: c("#859900"),
                trend_down: c("#dc322f"),
                contributor: c("#859900"),
                text: c("#586e75"),
                accent: c("#d33682"),
                ..Self::brand()
            },
        }
    }
}

fn render_header(svg: &mut String, days: &[DailyActivity], period: PeriodDays, palette: &Palette) {
    begin_section(svg, 0);
    panel(svg, 0, 0, 813, 47, palette);
    write!(
        svg,
        r##"<circle class="ping" opacity="0" cx="20" cy="24" r="9" fill="none" stroke="{0}" stroke-width="1.5"/><g transform="translate(10 16)" fill="{0}"><rect width="20" height="2" rx="1"/><rect y="14" width="20" height="2" rx="1"/><rect y="4" width="6" height="2" rx="1"/><rect x="7" y="4" width="6" height="2" rx="1"/><rect x="14" y="4" width="6" height="2" rx="1"/><rect y="8" width="6" height="4" rx="1"/><rect x="7" y="8" width="6" height="4" rx="1"/><rect x="14" y="8" width="6" height="4" rx="1"/></g>"##,
        palette.accent,
    )
    .unwrap();
    write!(
        svg,
        r##"<text x="38" y="29" class="sans small bold glow" fill="{}" stroke-width="0.5">{} Contributions in the Last {} Days</text>"##,
        palette.accent,
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
            r##"<rect class="cell w{}" data-header-day="{}" x="{:.1}" y="20" width="{:.1}" height="8" rx="1.5" opacity="{:.1}" fill="{}"/>"##,
            wave(index, activity.len()),
            index + 1,
            533.5 + index as f64 * slot,
            width,
            opacity(*value, max),
            palette.accent
        )
        .unwrap();
    }
    end_section(svg);
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
    palette: &Palette,
) {
    // Columns sit at x = 0, 274 and 548, so they enter as sections 1, 2 and 3.
    begin_section(svg, (x / 274) as usize + 1);
    panel(svg, x, 58, 264, 70, palette);
    let delta = current - previous;
    let (trend_color, bob) = if delta > 0.0 {
        (&palette.trend_up, "bob-up")
    } else {
        (&palette.trend_down, "bob-down")
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
        r#"<text x="{}" y="114" class="mono smaller bold" fill="{}">{}</text><g class="{}"><path d="{}" transform="translate({} 58)" stroke="{}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></g><text x="{}" y="114" text-anchor="end" class="mono smaller bold" fill="{}">{}</text>"#,
        x + 10,
        palette.secondary_text,
        format_delta(current, previous),
        bob,
        arrow,
        x,
        trend_color,
        x + 254,
        trend_color,
        comparison_label,
    )
    .unwrap();
    end_section(svg);
}

#[derive(Clone, Copy)]
struct DualChartStyle<'a> {
    title: &'a str,
    closed_color: &'a str,
    opened_color: &'a str,
    icon_from: &'a str,
    icon_to: &'a str,
}

impl<'a> DualChartStyle<'a> {
    fn new(title: &'a str, series: &'a Series) -> Self {
        Self {
            title,
            closed_color: &series.main,
            opened_color: &series.light,
            icon_from: &series.icon_from,
            icon_to: &series.icon_to,
        }
    }
}

fn render_dual_chart<F>(
    svg: &mut String,
    x: u32,
    step: usize,
    style: DualChartStyle<'_>,
    days: &[DailyActivity],
    values: F,
    palette: &Palette,
) where
    F: Fn(&DailyActivity) -> (u32, u32),
{
    begin_section(svg, step);
    panel(svg, x, 138, 264, 120, palette);
    if x == 0 {
        write!(
            svg,
            r##"<g transform="translate(10 158)"><path d="M8 9.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z" fill="url(#issue-dot)"/><path fill-rule="evenodd" clip-rule="evenodd" d="M8 0a8 8 0 1 0 0 16A8 8 0 0 0 8 0ZM1.5 8a6.5 6.5 0 1 1 13 0 6.5 6.5 0 0 1-13 0Z" fill="url(#issue-ring)"/><defs><linearGradient id="issue-dot" x1="8" y1="6.5" x2="8" y2="9.5" gradientUnits="userSpaceOnUse"><stop stop-color="{0}"/><stop offset=".729" stop-color="{1}"/></linearGradient><linearGradient id="issue-ring" x1="8" y1="0" x2="8" y2="16" gradientUnits="userSpaceOnUse"><stop stop-color="{0}"/><stop offset=".729" stop-color="{1}"/></linearGradient></defs></g>"##,
            style.icon_from,
            style.icon_to,
        )
        .unwrap();
    } else {
        write!(
            svg,
            r##"<g transform="translate(284 158)"><path fill-rule="evenodd" clip-rule="evenodd" d="M6.176 3.073 8.572.677A.25.25 0 0 1 9 .854v4.792a.25.25 0 0 1-.427.177L6.176 3.427a.25.25 0 0 1 0-.354ZM2.749 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm-2.25.75a2.25 2.25 0 1 1 3 2.122v5.256a2.25 2.25 0 1 1-1.5 0V5.372a2.25 2.25 0 0 1-1.5-2.122ZM10 2.5H9V4h1a1 1 0 0 1 1 1v5.628a2.25 2.25 0 1 0 1.5 0V5A2.5 2.5 0 0 0 10 2.5Zm1 10.25a.75.75 0 1 1 1.5 0 .75.75 0 0 1-1.5 0ZM2.75 12a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Z" fill="url(#pull-icon)"/><defs><linearGradient id="pull-icon" x1="0" y1="0" x2="0" y2="16" gradientUnits="userSpaceOnUse"><stop stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient></defs></g>"##,
            style.icon_from,
            style.icon_to,
        )
        .unwrap();
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
    end_section(svg);
}

fn render_push_chart(svg: &mut String, days: &[DailyActivity], palette: &Palette) {
    let x = 548;
    begin_section(svg, 6);
    panel(svg, x, 138, 264, 120, palette);
    write!(
        svg,
        r##"<g transform="translate(558 158)"><path fill-rule="evenodd" clip-rule="evenodd" d="M1 2.5A2.5 2.5 0 0 1 3.5 0h8.75a.75.75 0 0 1 .75.75v3.5a.75.75 0 0 1-1.5 0V1.5h-8a1 1 0 0 0-1 1v6.708A2.492 2.492 0 0 1 3.5 9h3.25a.75.75 0 0 1 0 1.5H3.5a1 1 0 1 0 0 2h5.75a.75.75 0 0 1 0 1.5H3.5A2.5 2.5 0 0 1 1 11.5v-9zm13.23 7.79a.75.75 0 0 0 1.06-1.06l-2.505-2.505a.75.75 0 0 0-1.06 0L9.22 9.229a.75.75 0 0 0 1.06 1.061l1.225-1.224v6.184a.75.75 0 0 0 1.5 0V9.066l1.224 1.224z" fill="url(#push-icon)"/><defs><linearGradient id="push-icon" x1="0" y1="0" x2="0" y2="16" gradientUnits="userSpaceOnUse"><stop stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient></defs></g><text x="582" y="171" class="sans small bold" fill="{}">Pushes</text><circle cx="743" cy="158" r="5" fill="{}"/><text x="753" y="162" class="sans smaller" fill="{}">Pushes</text>"##,
        palette.pushes.icon_from,
        palette.pushes.icon_to,
        palette.pushes.main,
        palette.pushes.light,
        palette.pushes.light,
    )
    .unwrap();
    let max = days.iter().map(|day| day.pushes).max().unwrap_or(0);
    for (index, day) in days.iter().enumerate() {
        chart_bar(
            svg,
            x,
            days.len(),
            index,
            day.pushes,
            max,
            BarStyle::new(&palette.pushes.light, 0.88, 0.0),
        );
    }
    end_section(svg);
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
        .set("class", format!("bar w{}", wave(index, days)))
        .set("fill", style.color);
    svg.push_str(&rectangle.to_string());
}

fn render_contributor_strip(
    svg: &mut String,
    contributors: &[ContributorActivity],
    period: PeriodDays,
    palette: &Palette,
) {
    begin_section(svg, 7);
    panel(svg, 0, 270, 813, 47, palette);
    write!(
        svg,
        r##"<g transform="translate(10 286)"><path d="M9.795 12.972c-.326.205-.6.364-.794.474a22.2 22.2 0 0 1-3.278-2.281C3.883 9.598 2.241 7.562 2.241 5.374c0-1.833 1.502-3.133 2.931-3.133 1.453 0 2.638.956 3.169 2.547a.695.695 0 0 0 1.318 0c.531-1.591 1.715-2.547 3.169-2.547 1.429 0 2.931 1.3 2.931 3.133 0 2.188-1.642 4.224-3.482 5.791-.899.766-1.802 1.382-2.482 1.807ZM9.345 14.843a25.4 25.4 0 0 0 3.833-2.619c1.887-1.606 3.972-4.002 3.972-6.85C17.15 2.773 15.025.85 12.828.85A4.41 4.41 0 0 0 9 2.976 4.412 4.412 0 0 0 5.172.85C2.975.85.85 2.774.85 5.374c0 2.848 2.084 5.244 3.972 6.85a25.4 25.4 0 0 0 3.728 2.562.695.695 0 0 0 .45.148.696.696 0 0 0 .345-.091Z" fill="url(#heart-icon)"/><defs><linearGradient id="heart-icon" x1="9" y1="1" x2="9" y2="14.784" gradientUnits="userSpaceOnUse"><stop stop-color="{}"/><stop offset=".729" stop-color="{}"/></linearGradient></defs></g>"##,
        palette.heart_from, palette.heart_to,
    )
    .unwrap();
    svg.push_str(
        &Text::new("Top Contributors")
            .set("x", 36)
            .set("y", 299)
            .set("class", "sans small bold")
            .set("fill", palette.contributor.to_string())
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
            .set("fill", palette.contributor.to_string());
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
                .set("class", format!("cell w{}", bucket % 15))
                .set("opacity", opacity(value, global_max))
                .set("fill", palette.contributor.to_string());
            svg.push_str(&rectangle.to_string());
        }
    }
    end_section(svg);
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

fn panel(svg: &mut String, x: u32, y: u32, width: u32, height: u32, palette: &Palette) {
    let panel = Rectangle::new()
        .set("x", f64::from(x) + 0.5)
        .set("y", f64::from(y) + 0.5)
        .set("width", width)
        .set("height", height)
        .set("rx", 4.5)
        .set("fill", palette.panel_fill.to_string())
        .set("stroke", palette.panel_stroke.to_string());
    let highlight = Rectangle::new()
        .set("x", f64::from(x) + 0.5)
        .set("y", f64::from(y) + 0.5)
        .set("width", width)
        .set("height", height)
        .set("rx", 4.5)
        .set("fill", "none")
        .set("stroke", palette.panel_highlight.to_string());
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
