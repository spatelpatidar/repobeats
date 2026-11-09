//! Additional card types: languages, contributors and overview.
//!
//! These share the palette, fonts and panel style of the activity card and
//! scale with `size` like it does. All dynamic text goes through `label`, which
//! escapes it.

use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use svg::{
    Document,
    node::{
        Blob,
        element::{Rectangle, Style, Text},
    },
};

use super::{
    CardOptions, CardRenderError, Palette, begin_section, color_comment, end_section, opacity,
    panel, style, truncate_name,
};
use crate::{
    domain::RepositoryName,
    model::{ActivitySnapshot, RepoOverview},
};

const WIDTH: u32 = 814;
const INNER: u32 = 813;

/// Wraps `body` in an SVG whose pixel size follows `options.size` and whose
/// height follows the content (`view_height` in the 814-wide coordinate space).
fn finish(body: String, view_height: u32, options: CardOptions, palette: &Palette) -> String {
    let (width, _) = options.size.dimensions();
    let height = (f64::from(width) * f64::from(view_height) / f64::from(WIDTH)).round() as u32;
    Document::new()
        .set("width", width)
        .set("height", height)
        .set("viewBox", (0, 0, WIDTH, view_height))
        .set("fill", "none")
        .add(color_comment(&options))
        .add(Style::new(style(options.animate)))
        .add(
            Rectangle::new()
                .set("x", 0.5)
                .set("y", 0.5)
                .set("width", INNER)
                .set("height", view_height.saturating_sub(1))
                .set("fill", palette.background.to_string()),
        )
        .add(Blob::new(body))
        .to_string()
}

struct Label<'a> {
    x: u32,
    y: u32,
    class: &'a str,
    fill: &'a str,
    end: bool,
    size: Option<u32>,
}

fn label(svg: &mut String, spec: Label<'_>, content: impl Into<String>) {
    let mut text = Text::new(content.into())
        .set("x", spec.x)
        .set("y", spec.y)
        .set("class", spec.class)
        .set("fill", spec.fill);
    if spec.end {
        text = text.set("text-anchor", "end");
    }
    if let Some(size) = spec.size {
        text = text.set("font-size", size);
    }
    svg.push_str(&text.to_string());
}

fn plain<'a>(x: u32, y: u32, class: &'a str, fill: &'a str) -> Label<'a> {
    Label {
        x,
        y,
        class,
        fill,
        end: false,
        size: None,
    }
}

fn ellipsize(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        format!(
            "{}…",
            text.chars().take(max - 1).collect::<String>().trim_end()
        )
    }
}

/// 950 -> "950", 1_234 -> "1.2k", 12_345 -> "12.3k", 1_250_000 -> "1.3M".
fn compact(value: u64) -> String {
    fn one_decimal(number: f64, suffix: &str) -> String {
        let text = format!("{number:.1}");
        format!("{}{suffix}", text.strip_suffix(".0").unwrap_or(&text))
    }
    match value {
        0..=999 => value.to_string(),
        1_000..=999_949 => one_decimal(value as f64 / 1_000.0, "k"),
        _ => one_decimal(value as f64 / 1_000_000.0, "M"),
    }
}

fn relative(from: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let days = (now - from).num_days();
    match days {
        i64::MIN..=0 => "today".to_owned(),
        1 => "yesterday".to_owned(),
        2..=59 => format!("{days} days ago"),
        60..=729 => format!("{} months ago", days / 30),
        _ => format!("{} years ago", days / 365),
    }
}

// ---------------------------------------------------------------- languages

fn language_color(name: &str) -> &'static str {
    match name {
        "Rust" => "#dea584",
        "Python" => "#3572A5",
        "JavaScript" => "#f1e05a",
        "TypeScript" => "#3178c6",
        "Go" => "#00ADD8",
        "Java" => "#b07219",
        "Kotlin" => "#A97BFF",
        "Swift" => "#F05138",
        "C" => "#555555",
        "C++" => "#f34b7d",
        "C#" => "#178600",
        "Ruby" => "#701516",
        "PHP" => "#4F5D95",
        "HTML" => "#e34c26",
        "CSS" => "#563d7c",
        "SCSS" => "#c6538c",
        "Shell" => "#89e051",
        "Dockerfile" => "#384d54",
        "Makefile" => "#427819",
        "Dart" => "#00B4AB",
        "Vue" => "#41b883",
        "Svelte" => "#ff3e00",
        "Lua" => "#000080",
        "Scala" => "#c22d40",
        "Elixir" => "#6e4a7e",
        "Haskell" => "#5e5086",
        "Jupyter Notebook" => "#DA5B0B",
        "Nix" => "#7e7eff",
        "HCL" => "#844FBA",
        "PLpgSQL" | "SQL" => "#e38c00",
        "Protocol Buffer" => "#4a90e2",
        "Other" => "#8b949e",
        other => {
            const FALLBACK: [&str; 8] = [
                "#e57373", "#64b5f6", "#81c784", "#ffb74d", "#ba68c8", "#4db6ac", "#f06292",
                "#a1887f",
            ];
            FALLBACK[other.bytes().map(usize::from).sum::<usize>() % FALLBACK.len()]
        }
    }
}

fn percent(part: u64, total: u64) -> String {
    let value = part as f64 * 100.0 / total as f64;
    if value < 0.1 {
        "<0.1%".to_owned()
    } else {
        format!("{value:.1}%")
    }
}

/// Language breakdown. Shows the 8 largest languages and groups the rest as "Other".
pub fn render_languages(
    repository: &RepositoryName,
    languages: &[(String, u64)],
    options: CardOptions,
) -> String {
    const SHOWN: usize = 8;
    const COLUMNS: usize = 4;
    let palette = &Palette::resolve(&options);
    let total: u64 = languages.iter().map(|(_, bytes)| bytes).sum();
    let mut body = String::new();

    if total == 0 {
        begin_section(&mut body, 0);
        panel(&mut body, 0, 0, INNER, 47, palette);
        label(
            &mut body,
            plain(12, 29, "sans small bold", &palette.accent),
            "Languages",
        );
        label(
            &mut body,
            Label {
                end: true,
                ..plain(801, 29, "mono smaller", &palette.secondary_text)
            },
            format!("No language data for {repository}"),
        );
        end_section(&mut body);
        return finish(body, 48, options, palette);
    }

    let mut shown: Vec<(String, u64)> = languages.iter().take(SHOWN).cloned().collect();
    let rest: u64 = languages.iter().skip(SHOWN).map(|(_, bytes)| bytes).sum();
    if rest > 0 {
        shown.push(("Other".to_owned(), rest));
    }
    let rows = shown.len().div_ceil(COLUMNS) as u32;
    let panel_height = 82 + (rows - 1) * 24 + 18;
    begin_section(&mut body, 0);
    panel(&mut body, 0, 0, INNER, panel_height, palette);
    label(
        &mut body,
        plain(12, 29, "sans small bold", &palette.accent),
        "Languages",
    );
    label(
        &mut body,
        Label {
            end: true,
            ..plain(801, 29, "mono smaller", &palette.secondary_text)
        },
        repository.to_string(),
    );
    end_section(&mut body);

    body.push_str(
        r#"<clipPath id="lang-clip"><rect x="12" y="44" width="789" height="10" rx="5"/></clipPath><g clip-path="url(#lang-clip)"><g class="wipe w0">"#,
    );
    let mut cursor = 12.0_f64;
    for (name, bytes) in &shown {
        let width = *bytes as f64 / total as f64 * 789.0;
        write!(
            body,
            r#"<rect x="{cursor:.2}" y="44" width="{width:.2}" height="10" fill="{}"/>"#,
            language_color(name)
        )
        .unwrap();
        cursor += width;
    }
    body.push_str("</g></g>");

    for (index, (name, bytes)) in shown.iter().enumerate() {
        let x = 12 + (index % COLUMNS) as u32 * 199;
        let y = 82 + (index / COLUMNS) as u32 * 24;
        begin_section(&mut body, index + 2);
        write!(
            body,
            r#"<circle cx="{}" cy="{}" r="5" fill="{}"/>"#,
            x + 5,
            y - 4,
            language_color(name)
        )
        .unwrap();
        label(
            &mut body,
            plain(x + 16, y, "sans smaller", &palette.text),
            ellipsize(name, 13),
        );
        label(
            &mut body,
            Label {
                end: true,
                ..plain(x + 178, y, "mono smaller", &palette.secondary_text)
            },
            percent(*bytes, total),
        );
        end_section(&mut body);
    }
    finish(body, panel_height + 1, options, palette)
}

// ------------------------------------------------------------- contributors

/// Ranked contributors for the selected period, one bar per person (top 10).
pub fn render_contributors(
    snapshot: &ActivitySnapshot,
    options: CardOptions,
) -> Result<String, CardRenderError> {
    const MAX_ROWS: usize = 10;
    let days = options.period.get();
    if snapshot.current.len() < days {
        return Err(CardRenderError(days));
    }
    let palette = &Palette::resolve(&options);
    let mut ranked = snapshot
        .contributors
        .iter()
        .filter_map(|contributor| {
            let start = contributor.days.len().checked_sub(days)?;
            let total: u32 = contributor.days[start..].iter().sum();
            (total > 0).then_some((contributor.name.as_str(), total))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
    let people = ranked.len();
    let all: u32 = ranked.iter().map(|(_, total)| total).sum();
    ranked.truncate(MAX_ROWS);

    let mut body = String::new();
    begin_section(&mut body, 0);
    panel(&mut body, 0, 0, INNER, 47, palette);
    label(
        &mut body,
        plain(12, 29, "sans small bold", &palette.accent),
        "Top Contributors",
    );
    label(
        &mut body,
        Label {
            end: true,
            ..plain(801, 29, "mono smaller", &palette.secondary_text)
        },
        format!(
            "{people} {} · {all} contributions · last {days} days",
            if people == 1 {
                "contributor"
            } else {
                "contributors"
            }
        ),
    );
    end_section(&mut body);

    if ranked.is_empty() {
        begin_section(&mut body, 1);
        panel(&mut body, 0, 58, INNER, 40, palette);
        label(
            &mut body,
            plain(12, 83, "sans smaller", &palette.secondary_text),
            "No activity in this period",
        );
        end_section(&mut body);
        return Ok(finish(body, 99, options, palette));
    }

    let rows = ranked.len() as u32;
    let panel_height = rows * 24 + 12;
    begin_section(&mut body, 1);
    panel(&mut body, 0, 58, INNER, panel_height, palette);
    end_section(&mut body);
    let max = ranked[0].1;
    for (index, (name, total)) in ranked.iter().enumerate() {
        let y = 58 + 6 + index as u32 * 24;
        begin_section(&mut body, index + 2);
        label(
            &mut body,
            plain(12, y + 15, "mono bold smaller", &palette.contributor),
            truncate_name(name),
        );
        let width = (f64::from(*total) / f64::from(max) * 500.0).max(3.0);
        write!(
            body,
            r#"<rect class="hbar w{}" x="200" y="{}" width="{width:.1}" height="10" rx="2" fill="{}" opacity="{:.2}"/>"#,
            index.min(15),
            y + 5,
            palette.contributor,
            opacity(*total, max).max(0.45),
        )
        .unwrap();
        label(
            &mut body,
            Label {
                end: true,
                ..plain(801, y + 15, "mono smaller", &palette.secondary_text)
            },
            format!(
                "{total} ({}%)",
                trim_percent(f64::from(*total) * 100.0 / f64::from(all))
            ),
        );
        end_section(&mut body);
    }
    Ok(finish(body, 58 + panel_height + 1, options, palette))
}

fn trim_percent(value: f64) -> String {
    let text = format!("{value:.1}");
    text.strip_suffix(".0").unwrap_or(&text).to_owned()
}

// ----------------------------------------------------------------- overview

struct Tile<'a> {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    stripe: &'a str,
    name: &'a str,
    value: String,
    value_size: u32,
    step: usize,
}

fn tile(svg: &mut String, tile: Tile<'_>, palette: &Palette) {
    begin_section(svg, tile.step);
    panel(svg, tile.x, tile.y, tile.width, tile.height, palette);
    write!(
        svg,
        r#"<rect x="{}" y="{}" width="3" height="{}" rx="1.5" fill="{}"/>"#,
        tile.x + 8,
        tile.y + 10,
        tile.height - 20,
        tile.stripe
    )
    .unwrap();
    label(
        svg,
        plain(
            tile.x + 20,
            tile.y + 24,
            "sans smaller",
            &palette.secondary_text,
        ),
        tile.name,
    );
    label(
        svg,
        Label {
            size: Some(tile.value_size),
            ..plain(
                tile.x + 20,
                tile.y + tile.height - 13,
                "sans bold",
                &palette.text,
            )
        },
        tile.value,
    );
    end_section(svg);
}

/// Stars, forks, open issues, watchers, license, language, last push and latest release.
pub fn render_overview(overview: &RepoOverview, options: CardOptions) -> String {
    let palette = &Palette::resolve(&options);
    let mut body = String::new();

    let header = if overview.description.is_some() {
        66
    } else {
        47
    };
    begin_section(&mut body, 0);
    panel(&mut body, 0, 0, INNER, header, palette);
    label(
        &mut body,
        plain(12, 29, "sans small bold", &palette.accent),
        overview.repository.to_string(),
    );
    if let Some(description) = &overview.description {
        label(
            &mut body,
            plain(12, 52, "sans smaller", &palette.secondary_text),
            ellipsize(description, 110),
        );
    }
    end_section(&mut body);

    let top = header + 10;
    let stripes = &palette.stripes;
    let first = [
        (&stripes[0], "Stars", compact(overview.stars)),
        (&stripes[1], "Forks", compact(overview.forks)),
        (&stripes[2], "Open issues", compact(overview.open_issues)),
        (&stripes[3], "Watchers", compact(overview.watchers)),
    ];
    for (index, (stripe, name, value)) in first.into_iter().enumerate() {
        tile(
            &mut body,
            Tile {
                x: index as u32 * 206,
                y: top,
                width: 195,
                height: 70,
                stripe,
                name,
                value,
                value_size: 26,
                step: index + 1,
            },
            palette,
        );
    }

    let second = [
        (
            &stripes[4],
            "License",
            overview
                .license
                .clone()
                .unwrap_or_else(|| "None".to_owned()),
        ),
        (
            &stripes[5],
            "Main language",
            overview
                .language
                .clone()
                .unwrap_or_else(|| "n/a".to_owned()),
        ),
        (
            &stripes[6],
            "Last push",
            overview.pushed_at.map_or_else(
                || "n/a".to_owned(),
                |at| relative(at, overview.generated_at),
            ),
        ),
        (
            &stripes[7],
            "Latest release",
            overview
                .latest_release
                .clone()
                .unwrap_or_else(|| "none".to_owned()),
        ),
    ];
    for (index, (stripe, name, value)) in second.into_iter().enumerate() {
        tile(
            &mut body,
            Tile {
                x: index as u32 * 206,
                y: top + 80,
                width: 195,
                height: 60,
                stripe,
                name,
                value: ellipsize(&value, 14),
                value_size: 16,
                step: index + 5,
            },
            palette,
        );
    }
    finish(body, top + 80 + 60 + 2, options, palette)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compacts_numbers() {
        assert_eq!(compact(0), "0");
        assert_eq!(compact(999), "999");
        assert_eq!(compact(1_000), "1k");
        assert_eq!(compact(1_234), "1.2k");
        assert_eq!(compact(12_345), "12.3k");
        assert_eq!(compact(999_949), "999.9k");
        assert_eq!(compact(1_260_000), "1.3M");
    }

    #[test]
    fn ellipsizes_on_character_boundaries() {
        assert_eq!(ellipsize("short", 10), "short");
        assert_eq!(ellipsize("abcdefghij", 5), "abcd…");
        assert_eq!(ellipsize("ééééééé", 4), "ééé…");
    }
}
