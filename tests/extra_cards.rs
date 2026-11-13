use std::str::FromStr;

use chrono::{Duration, TimeZone, Utc};
use repobeats::{
    card::{
        CardKind, CardOptions, CardRenderer, CardSize, CardTheme, PeriodDays, SvgCardRenderer,
        render_contributors, render_languages, render_overview,
    },
    domain::RepositoryName,
    model::{ActivitySnapshot, ContributorActivity, DailyActivity, RepoOverview, parse_languages},
};
use serde_json::json;

/// The svg crate puts line breaks around text content; SVG ignores them.
fn flat(svg: &str) -> String {
    svg.replace('\n', "")
}

fn repository() -> RepositoryName {
    RepositoryName::from_str("acme/widgets").unwrap()
}

fn options(theme: CardTheme) -> CardOptions {
    CardOptions {
        theme,
        ..CardOptions::default()
    }
}

fn snapshot() -> ActivitySnapshot {
    let mut recent = vec![0; 365];
    recent[360..].fill(2); // 10 contributions in the last 5 days
    let mut old = vec![0; 365];
    old[10] = 50; // lots of activity, but long ago
    let mut mid = vec![0; 365];
    mid[340] = 4;
    mid[364] = 1;
    ActivitySnapshot {
        repository: repository(),
        generated_at: Utc.with_ymd_and_hms(2026, 11, 3, 0, 0, 0).unwrap(),
        current: vec![DailyActivity::default(); 365],
        previous: vec![DailyActivity::default(); 365],
        contributors: vec![
            ContributorActivity {
                name: "old-timer".into(),
                total: 50,
                days: old,
            },
            ContributorActivity {
                name: "mid<dle>".into(),
                total: 5,
                days: mid,
            },
            ContributorActivity {
                name: "recent".into(),
                total: 10,
                days: recent,
            },
        ],
    }
}

#[test]
fn themes_parse_from_kebab_case_names() {
    for (name, theme) in [
        ("light", CardTheme::Light),
        ("dark", CardTheme::Dark),
        ("github-dimmed", CardTheme::GithubDimmed),
        ("dracula", CardTheme::Dracula),
        ("nord", CardTheme::Nord),
        ("tokyo-night", CardTheme::TokyoNight),
        ("solarized-light", CardTheme::SolarizedLight),
    ] {
        assert_eq!(
            serde_json::from_value::<CardTheme>(json!(name)).unwrap(),
            theme
        );
    }
    assert!(serde_json::from_value::<CardTheme>(json!("neon")).is_err());
    assert_eq!(CardTheme::ALL.len(), 7);
}

#[test]
fn card_kinds_parse_and_default_to_activity() {
    assert_eq!(CardKind::default(), CardKind::Activity);
    for (name, kind) in [
        ("activity", CardKind::Activity),
        ("languages", CardKind::Languages),
        ("contributors", CardKind::Contributors),
        ("overview", CardKind::Overview),
        ("graph", CardKind::Graph),
    ] {
        assert_eq!(
            serde_json::from_value::<CardKind>(json!(name)).unwrap(),
            kind
        );
    }
    assert!(serde_json::from_value::<CardKind>(json!("pie")).is_err());
}

#[test]
fn every_theme_renders_every_card_with_its_own_background() {
    let backgrounds = [
        (CardTheme::GithubDimmed, "#22272e"),
        (CardTheme::Dracula, "#282a36"),
        (CardTheme::Nord, "#2e3440"),
        (CardTheme::TokyoNight, "#1a1b26"),
        (CardTheme::SolarizedLight, "#fdf6e3"),
    ];
    let overview = RepoOverview::from_json(
        repository(),
        Utc::now(),
        &json!({"stargazers_count": 3}),
        None,
    );
    for theme in CardTheme::ALL {
        let options = options(theme);
        let cards = [
            SvgCardRenderer.render(&snapshot(), options).unwrap(),
            render_contributors(&snapshot(), options).unwrap(),
            render_languages(&repository(), &[("Rust".into(), 10)], options),
            render_overview(&overview, options),
        ];
        for svg in cards {
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        }
    }
    for (theme, color) in backgrounds {
        let svg = SvgCardRenderer.render(&snapshot(), options(theme)).unwrap();
        let svg = flat(&svg);
        assert!(svg.contains(&format!("fill=\"{color}\"")), "{theme:?}");
    }
    // Light and dark keep the original header color.
    let light = SvgCardRenderer
        .render(&snapshot(), options(CardTheme::Light))
        .unwrap();
    assert!(light.contains("fill=\"#ec4899\""));
}

#[test]
fn languages_card_shows_percentages_groups_the_tail_and_escapes_names() {
    let mut languages: Vec<(String, u64)> = vec![("Rust".into(), 6_000), ("A&B<x>".into(), 3_000)];
    for index in 0..8 {
        languages.push((format!("Tiny{index}"), 1));
    }
    let svg = render_languages(&repository(), &languages, options(CardTheme::Dark));
    let svg = flat(&svg);

    assert!(svg.contains(">Rust<"));
    assert!(svg.contains("66.6%"));
    assert!(svg.contains("33.3%"));
    assert!(svg.contains("A&amp;B&lt;x&gt;"));
    assert!(!svg.contains("A&B<x>"));
    assert!(svg.contains("&lt;0.1%"));
    assert!(
        svg.contains(">Other<"),
        "tail beyond 8 languages is grouped"
    );
    assert!(!svg.contains("Tiny7"));
    assert!(svg.contains("acme/widgets"));
}

#[test]
fn languages_card_handles_a_repository_without_code() {
    let svg = render_languages(&repository(), &[], CardOptions::default());
    let svg = flat(&svg);
    assert!(svg.contains("No language data for acme/widgets"));
}

#[test]
fn parse_languages_sorts_by_bytes_and_ignores_junk() {
    let parsed = parse_languages(&json!({"Go": 5, "Rust": 90, "Zig": 5, "Empty": 0, "Bad": "x"}));
    assert_eq!(
        parsed,
        vec![
            ("Rust".to_owned(), 90),
            ("Go".to_owned(), 5),
            ("Zig".to_owned(), 5)
        ]
    );
    assert!(parse_languages(&json!(null)).is_empty());
}

#[test]
fn contributors_card_ranks_by_the_selected_period() {
    let svg = render_contributors(&snapshot(), options(CardTheme::Light)).unwrap();
    let svg = flat(&svg);
    // 30-day window: recent (10) > mid (5); old-timer is outside the window.
    assert!(svg.contains("2 contributors · 15 contributions · last 30 days"));
    assert!(!svg.contains("old-timer"));
    let recent = svg.find(">recent<").unwrap();
    let middle = svg.find("mid&lt;dle&gt;").unwrap();
    assert!(recent < middle);
    assert!(svg.contains("10 (66.7%)"));
    assert!(svg.contains("5 (33.3%)"));

    // A year window includes everyone, ordered by total.
    let year = CardOptions {
        period: PeriodDays::YEAR,
        ..CardOptions::default()
    };
    let svg = render_contributors(&snapshot(), year).unwrap();
    let svg = flat(&svg);
    assert!(svg.contains("3 contributors · 65 contributions · last 365 days"));
    assert!(svg.find("old-timer").unwrap() < svg.find(">recent<").unwrap());
}

#[test]
fn contributors_card_reports_empty_periods_and_short_history() {
    let mut empty = snapshot();
    empty.contributors.clear();
    let svg = render_contributors(&empty, CardOptions::default()).unwrap();
    let svg = flat(&svg);
    assert!(svg.contains("No activity in this period"));

    let mut short = snapshot();
    short.current.truncate(10);
    assert!(render_contributors(&short, CardOptions::default()).is_err());
}

#[test]
fn contributors_card_lists_at_most_ten_people() {
    let mut many = snapshot();
    many.contributors = (0..14)
        .map(|index| {
            let mut days = vec![0; 365];
            days[364] = 20 - index;
            ContributorActivity {
                name: format!("user{index:02}"),
                total: 20 - index,
                days,
            }
        })
        .collect();
    let svg = render_contributors(&many, CardOptions::default()).unwrap();
    let svg = flat(&svg);
    assert!(svg.contains("14 contributors"));
    assert!(svg.contains("user09") && !svg.contains("user10"));
}

#[test]
fn overview_card_formats_counts_and_dates() {
    let now = Utc.with_ymd_and_hms(2026, 11, 3, 12, 0, 0).unwrap();
    let body = json!({
        "description": "Fast <widgets> & more",
        "stargazers_count": 12_345,
        "forks_count": 987,
        "open_issues_count": 42,
        "subscribers_count": 1_500_000,
        "license": {"spdx_id": "MIT", "name": "MIT License"},
        "language": "Rust",
        "pushed_at": (now - Duration::days(3)).to_rfc3339(),
    });
    let overview = RepoOverview::from_json(repository(), now, &body, Some("v1.2.3".into()));
    let svg = render_overview(&overview, options(CardTheme::Nord));
    let svg = flat(&svg);

    for expected in [
        "acme/widgets",
        "Fast &lt;widgets&gt; &amp; more",
        ">12.3k<",
        ">987<",
        ">42<",
        ">1.5M<",
        ">MIT<",
        ">Rust<",
        ">3 days ago<",
        ">v1.2.3<",
    ] {
        assert!(svg.contains(expected), "missing {expected}");
    }
}

#[test]
fn overview_falls_back_gracefully_when_fields_are_missing() {
    let body = json!({"license": {"spdx_id": "NOASSERTION", "name": "Other"}});
    let overview = RepoOverview::from_json(repository(), Utc::now(), &body, None);
    assert_eq!(overview.license.as_deref(), Some("Other"));
    let svg = render_overview(&overview, CardOptions::default());
    let svg = flat(&svg);
    assert!(svg.contains(">none<"));
    assert!(svg.contains(">n/a<"));

    let none = RepoOverview::from_json(repository(), Utc::now(), &json!({}), None);
    assert!(flat(&render_overview(&none, CardOptions::default())).contains(">None<"));
}

#[test]
fn new_cards_scale_with_size() {
    for (size, width) in [
        (CardSize::Small, 610),
        (CardSize::Medium, 814),
        (CardSize::Large, 1221),
    ] {
        let svg = render_languages(
            &repository(),
            &[("Rust".into(), 1)],
            CardOptions {
                size,
                ..CardOptions::default()
            },
        );
        let svg = flat(&svg);
        assert!(svg.contains(&format!("width=\"{width}\"")));
        assert!(svg.contains("viewBox=\"0 0 814 "));
    }
}
