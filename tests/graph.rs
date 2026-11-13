use chrono::{NaiveDate, TimeZone, Utc};
use repobeats::{
    card::{
        Background, CardOptions, CardTheme, GraphError, GraphMetric, GraphOptions, GraphRange,
        render_graph, render_graph_error,
    },
    domain::RepositoryName,
    model::{ActivitySnapshot, DailyActivity},
    params::ParamError,
};

fn flat(svg: &str) -> String {
    svg.replace('\n', "")
}

/// The `d` attribute of the data line (the only path with `pathLength`).
fn line_path(svg: &str) -> &str {
    let marker = "pathLength=\"1\" d=\"";
    let start = svg.find(marker).expect("no line") + marker.len();
    &svg[start..start + svg[start..].find('"').unwrap()]
}

/// No `NaN` or infinity in the drawing. The style block is skipped because it
/// legitimately contains the word `infinite`.
fn assert_finite(svg: &str) {
    let drawing = svg.split("</style>").nth(1).expect("no style block");
    assert!(
        !drawing.contains("NaN") && !drawing.contains("inf"),
        "{drawing}"
    );
}

fn day(n: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, n).unwrap()
}

/// 365 days ending 2026-11-13. The last five days have 1, 2, 3, 4, 5 pushes
/// and, on the last day only, one issue and one pull request opened.
fn snapshot() -> ActivitySnapshot {
    let mut current = vec![DailyActivity::default(); 365];
    for (offset, pushes) in (1..=5).enumerate() {
        current[360 + offset].pushes = pushes;
    }
    current[364].issues_opened = 1;
    current[364].pulls_opened = 1;
    ActivitySnapshot {
        repository: "acme/widgets".parse::<RepositoryName>().unwrap(),
        generated_at: Utc.with_ymd_and_hms(2026, 11, 13, 9, 0, 0).unwrap(),
        current,
        previous: vec![DailyActivity::default(); 365],
        contributors: vec![],
    }
}

fn options(query: &str) -> GraphOptions {
    GraphOptions::from_query(query).unwrap()
}

fn render(query: &str) -> String {
    flat(&render_graph(&snapshot(), CardOptions::default(), &options(query)).unwrap())
}

#[test]
fn defaults() {
    let defaults = options("");
    assert_eq!(defaults.range, GraphRange::Last(31));
    assert_eq!(defaults.metric, GraphMetric::Contributions);
    assert!(defaults.grid && defaults.smooth && !defaults.area);
    assert_eq!(defaults.height, 318);
}

#[test]
fn parses_every_parameter() {
    let parsed = options(
        "days=14&metric=prs&area=true&grid=off&smooth=0&hide_title=yes&hide_border=1\
         &custom_title=Hello%20there&radius=8&height=400&bg_color=transparent\
         &border_color=%23ff0000&color=FFF&title_color=abc123&line=58a6ff&point=f78166&area_color=00ff00",
    );
    assert_eq!(parsed.range, GraphRange::Last(14));
    assert_eq!(parsed.metric, GraphMetric::PullRequests);
    assert!(
        parsed.area && !parsed.grid && !parsed.smooth && parsed.hide_title && parsed.hide_border
    );
    assert_eq!(parsed.title.as_deref(), Some("Hello there"));
    assert_eq!((parsed.radius, parsed.height), (8, 400));
    assert_eq!(parsed.colors.background, Some(Background::Transparent));
    assert!(parsed.colors.border.is_some() && parsed.colors.text.is_some());
    assert!(parsed.colors.line.is_some() && parsed.colors.area.is_some());
}

#[test]
fn radius_and_height_are_clamped() {
    let parsed = options("radius=99&height=5000");
    assert_eq!((parsed.radius, parsed.height), (16, 600));
    assert_eq!(options("height=1").height, 200);
}

#[test]
fn empty_and_unknown_parameters_are_ignored() {
    let parsed = options("days=&v=2&unknown=x&area=");
    assert_eq!(parsed, GraphOptions::default());
}

#[test]
fn bad_values_are_rejected() {
    for query in [
        "days=0",
        "days=366",
        "days=abc",
        "days=-1",
        "area=maybe",
        "line=zzz",
        "line=12345",
        "bg_color=url(x)",
        "metric=stars",
        "from=2026-13-01&to=2026-12-01",
        "radius=-1",
        "height=tall",
    ] {
        assert!(
            GraphOptions::from_query(query).is_err(),
            "{query} should fail"
        );
    }
}

#[test]
fn date_range_needs_both_ends_in_order() {
    assert_eq!(
        GraphOptions::from_query("from=2026-11-01").unwrap_err(),
        ParamError::Missing("to")
    );
    assert_eq!(
        GraphOptions::from_query("to=2026-11-01").unwrap_err(),
        ParamError::Missing("from")
    );
    assert!(matches!(
        GraphOptions::from_query("from=2026-11-05&to=2026-11-01"),
        Err(ParamError::Conflict(_))
    ));
    let parsed = options("days=5&from=2026-11-01&to=2026-11-03");
    assert_eq!(
        parsed.range,
        GraphRange::Between {
            from: day(1),
            to: day(3)
        }
    );
}

#[test]
fn lookback_reaches_the_start_of_the_range() {
    assert_eq!(options("days=7").lookback_days(day(13)), 7);
    assert_eq!(
        options("from=2026-11-01&to=2026-11-03").lookback_days(day(13)),
        13
    );
}

#[test]
fn counts_the_requested_metric() {
    assert!(render("days=5").contains("17 contributions · Nov 9 – Nov 13"));
    assert!(render("days=5&metric=pushes").contains("15 pushes · Nov 9 – Nov 13"));
    assert!(render("days=5&metric=issues").contains("1 issue opened"));
    assert!(render("days=5&metric=pulls").contains("1 pull request opened"));
}

#[test]
fn date_range_selects_those_days() {
    let svg = render("from=2026-11-10&to=2026-11-12");
    assert!(svg.contains("9 contributions · Nov 10 – Nov 12"), "{svg}");
}

#[test]
fn single_day_draws_without_a_line() {
    let svg = render("days=1");
    assert!(svg.contains("<circle"));
    assert!(!svg.contains("pathLength"));
    assert_finite(&svg);
}

#[test]
fn empty_activity_still_has_a_scale() {
    let mut empty = snapshot();
    empty.current = vec![DailyActivity::default(); 365];
    let svg = flat(&render_graph(&empty, CardOptions::default(), &options("")).unwrap());
    assert!(svg.contains("0 contributions"));
    assert_finite(&svg);
}

#[test]
fn out_of_window_requests_fail() {
    let graph = options("from=2026-11-10&to=2026-11-20");
    assert!(matches!(
        render_graph(&snapshot(), CardOptions::default(), &graph),
        Err(GraphError::OutsideWindow { .. })
    ));
    let mut short = snapshot();
    short.current.truncate(10);
    assert_eq!(
        render_graph(&short, CardOptions::default(), &options("days=31")),
        Err(GraphError::NotEnoughHistory(31))
    );
}

#[test]
fn switches_change_the_output() {
    assert!(render("days=5&area=true").contains("repobeats-area)"));
    assert!(!render("days=5").contains("url(#repobeats-area)"));
    assert!(!render("days=5&hide_title=true").contains("acme/widgets"));
    let straight = render("days=5&smooth=false");
    assert!(line_path(&straight).contains(" L") && !line_path(&straight).contains(" C"));
    let curved = render("days=5");
    assert!(line_path(&curved).contains(" C") && !line_path(&curved).contains(" L"));
    let static_svg = flat(
        &render_graph(
            &snapshot(),
            CardOptions {
                animate: false,
                ..CardOptions::default()
            },
            &options("days=5"),
        )
        .unwrap(),
    );
    assert!(!static_svg.contains("@keyframes"));
}

#[test]
fn colors_and_theme_apply() {
    let svg = render(
        "days=5&bg_color=010203&line=0a0b0c&point=0d0e0f&border_color=ff0000&hide_title=true",
    );
    for color in ["#010203", "#0a0b0c", "#0d0e0f", "#ff0000"] {
        assert!(svg.contains(color), "missing {color}");
    }
    assert!(render("days=5&hide_border=true").contains(r#"stroke="none""#));
    let dark = flat(
        &render_graph(
            &snapshot(),
            CardOptions {
                theme: CardTheme::Dracula,
                ..CardOptions::default()
            },
            &options("days=5"),
        )
        .unwrap(),
    );
    assert!(dark.contains("#282a36"));
}

#[test]
fn custom_title_is_escaped_and_shortened() {
    let svg = render("days=5&custom_title=%3Cscript%3Ealert(1)%3C/script%3E%26%22x");
    assert!(!svg.contains("<script"));
    assert!(svg.contains("&lt;script&gt;"));
    let long = render(&format!("days=5&custom_title={}", "a".repeat(500)));
    assert!(!long.contains(&"a".repeat(100)));
}

#[test]
fn error_card_escapes_the_message() {
    let svg = render_graph_error("bad <img src=x onerror=alert(1)> value");
    assert!(!svg.contains("<img"));
    assert!(svg.contains("&lt;img"));
}
