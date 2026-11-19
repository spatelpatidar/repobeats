use chrono::{Duration, NaiveDate};
use repobeats::{
    card::{
        Background, CardOptions, CardTheme, GraphError, GraphOptions, GraphRange, LabelMode,
        graph_dates, render_graph, render_graph_error,
    },
    domain::parse_username,
    model::ContributionCalendar,
    params::ParamError,
};
use serde_json::json;

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

fn day(month: u32, n: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, month, n).unwrap()
}

fn today() -> NaiveDate {
    day(11, 13)
}

/// 365 days ending today. The last five days have 1, 2, 3, 4, 5 contributions.
fn calendar() -> ContributionCalendar {
    let days = (0..365)
        .map(|back| {
            let date = today() - Duration::days(364 - back);
            let count = if back >= 360 { (back - 359) as u32 } else { 0 };
            (date, count)
        })
        .collect();
    ContributionCalendar {
        login: "octocat".into(),
        name: Some("The Octocat".into()),
        days,
    }
}

/// Like `render`, without the axis titles, so only date labels are counted.
fn render_plain(query: &str) -> String {
    render(&format!("{query}&hide_axis_titles=true"))
}

fn options(query: &str) -> GraphOptions {
    GraphOptions::from_query(query).unwrap()
}

fn render(query: &str) -> String {
    flat(
        &render_graph(
            &calendar(),
            today(),
            CardOptions::default(),
            &options(query),
        )
        .unwrap(),
    )
}

#[test]
fn defaults() {
    let defaults = options("");
    assert_eq!(defaults.range, GraphRange::Last(31));
    assert!(defaults.grid && defaults.smooth && !defaults.area);
    assert_eq!(defaults.height, 318);
}

#[test]
fn parses_every_parameter() {
    let parsed = options(
        "days=14&area=true&grid=off&smooth=0&hide_title=yes&hide_border=1\
         &custom_title=Hello%20there&radius=8&height=400&bg_color=transparent\
         &border_color=%23ff0000&color=FFF&title_color=abc123&line=58a6ff&point=f78166&area_color=00ff00",
    );
    assert_eq!(parsed.range, GraphRange::Last(14));
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
    let parsed = options("days=&v=2&unknown=x&area=&username=octocat&theme=dark");
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
            from: day(11, 1),
            to: day(11, 3)
        }
    );
}

#[test]
fn dates_come_from_days_or_the_range() {
    assert_eq!(
        graph_dates(&options("days=7"), today()),
        Ok((day(11, 7), day(11, 13)))
    );
    assert_eq!(
        graph_dates(&options("from=2026-11-01&to=2026-11-03"), today()),
        Ok((day(11, 1), day(11, 3)))
    );
    // 365 days back from 2026-11-13 is 2025-11-14.
    assert!(graph_dates(&options("from=2025-11-14&to=2026-11-13"), today()).is_ok());
    for query in [
        "from=2025-11-13&to=2026-11-13",
        "from=2026-11-10&to=2026-11-14",
    ] {
        assert!(matches!(
            graph_dates(&options(query), today()),
            Err(GraphError::OutsideWindow { .. })
        ));
    }
}

#[test]
fn draws_the_daily_counts() {
    let svg = render("days=5");
    assert!(svg.contains("15 contributions · Nov 9 – Nov 13"), "{svg}");
    // The point for each day carries its count.
    for (date, count) in [("Nov 9", 1), ("Nov 11", 3), ("Nov 13", 5)] {
        assert!(svg.contains(&format!("<title>{date}: {count}</title>")));
    }
}

/// Labels written under the axis: horizontal ones are centered on their day,
/// sideways ones are rotated.
fn axis_labels(svg: &str) -> usize {
    svg.matches("text-anchor=\"middle\"").count() + svg.matches("transform=\"rotate(").count()
}

fn vertical_grid_lines(svg: &str) -> usize {
    svg.matches("class=\"vgrid\"").count()
}

fn points(svg: &str) -> usize {
    svg.matches("<circle").count()
}

#[test]
fn every_day_is_labeled_while_there_is_room() {
    // Narrow slots: the day of the month, for every one of the 31 days.
    let month = render_plain("days=31");
    assert_eq!(axis_labels(&month), 31);
    for number in ["14", "30", "31", "1", "13"] {
        assert!(month.contains(&format!(">{number}</text>")), "{number}");
    }
    // Wide slots: the full date, for every day.
    let fortnight = render_plain("days=14");
    assert_eq!(axis_labels(&fortnight), 14);
    assert!(fortnight.contains(">Nov 9</text>") && fortnight.contains(">Nov 13</text>"));
    // Two months: still a label for every day, in smaller type.
    let two_months = render_plain("days=60");
    assert_eq!(axis_labels(&two_months), 60);
    assert!(two_months.contains("class=\"sans tiny\""));
}

#[test]
fn long_ranges_write_a_date_sideways_under_every_day() {
    let quarter = render_plain("days=90");
    assert_eq!(axis_labels(&quarter), 90);
    assert_eq!(quarter.matches("transform=\"rotate(-90").count(), 90);
    assert!(quarter.contains(">Aug 16</text>") && quarter.contains(">Nov 13</text>"));
}

#[test]
fn grid_lines_and_points_are_daily_as_long_as_they_fit() {
    for days in [14, 31, 60, 90, 120] {
        let svg = render_plain(&format!("days={days}"));
        assert_eq!(vertical_grid_lines(&svg), days, "grid, {days} days");
        assert_eq!(points(&svg), days, "points, {days} days");
    }
    // A year: too dense for one line or point per day. Labels are spaced out.
    let year = render_plain("days=365");
    assert!(vertical_grid_lines(&year) < 100);
    assert_eq!(points(&year), 0);
    let labels = axis_labels(&year);
    assert!((5..40).contains(&labels), "{labels}");
    assert!(year.contains(">Nov 13 '26</text>"));
    assert_finite(&year);
}

#[test]
fn axis_titles_are_drawn_and_can_be_hidden() {
    let svg = render("days=31");
    assert!(svg.contains(">Days</text>") && svg.contains(">Contributions</text>"));
    // The vertical title is written sideways, the horizontal one is not.
    assert!(svg.contains("rotate(-90 16.0"));
    let plain = render("days=31&hide_axis_titles=true");
    assert!(!plain.contains(">Days</text>") && !plain.contains(">Contributions</text>"));
    assert!(options("hide_axis_titles=1").hide_axis_titles);
    assert!(!options("").hide_axis_titles);
    for query in ["days=14", "days=60", "days=90", "days=365", "height=200"] {
        assert_finite(&render(query));
    }
}

#[test]
fn labels_option_picks_the_style() {
    for (value, mode) in [
        ("auto", LabelMode::Auto),
        ("date", LabelMode::Date),
        ("DATES", LabelMode::Date),
        ("number", LabelMode::Number),
        ("days", LabelMode::Number),
        ("sideways", LabelMode::Sideways),
        ("rotated", LabelMode::Sideways),
        ("none", LabelMode::None),
    ] {
        assert_eq!(options(&format!("labels={value}")).labels, mode, "{value}");
    }
    assert_eq!(options("").labels, LabelMode::Auto);
    assert!(GraphOptions::from_query("labels=weekly").is_err());
}

#[test]
fn each_label_style_is_drawn() {
    // 31 days: day numbers and sideways dates fit one under every day.
    let numbers = render_plain("days=31&labels=number");
    assert_eq!(axis_labels(&numbers), 31);
    assert!(numbers.contains(">14</text>") && numbers.contains(">13</text>"));
    let sideways = render_plain("days=31&labels=sideways");
    assert_eq!(axis_labels(&sideways), 31);
    assert_eq!(sideways.matches("transform=\"rotate(-90").count(), 31);
    // Full dates do not fit 31 across, so they are spaced out, newest last.
    let dates = render_plain("days=31&labels=date");
    assert!(
        (5..20).contains(&axis_labels(&dates)),
        "{}",
        axis_labels(&dates)
    );
    assert!(dates.contains(">Nov 13</text>"));
    // No labels: the grid is still daily.
    let none = render_plain("days=31&labels=none");
    assert_eq!(axis_labels(&none), 0);
    assert_eq!(vertical_grid_lines(&none), 31);
    // Day numbers past the width they fit in are spaced out, not overlapped.
    let crowded = render_plain("days=90&labels=number");
    assert!((20..90).contains(&axis_labels(&crowded)));
    // `auto` is the default.
    assert_eq!(render("days=31"), render("days=31&labels=auto"));
    for mode in ["date", "number", "sideways", "none"] {
        for days in [1, 7, 31, 90, 365] {
            assert_finite(&render(&format!("days={days}&labels={mode}")));
        }
    }
}

#[test]
fn grid_can_be_turned_off() {
    assert_eq!(vertical_grid_lines(&render("days=31&grid=false")), 0);
}

#[test]
fn date_range_selects_those_days() {
    let svg = render("from=2026-11-10&to=2026-11-12");
    assert!(svg.contains("9 contributions · Nov 10 – Nov 12"), "{svg}");
}

#[test]
fn days_missing_from_the_calendar_count_as_zero() {
    let mut sparse = calendar();
    sparse.days.retain(|(date, _)| *date != day(11, 12));
    let svg =
        flat(&render_graph(&sparse, today(), CardOptions::default(), &options("days=5")).unwrap());
    assert!(svg.contains("11 contributions"), "{svg}");
    assert!(svg.contains("<title>Nov 12: 0</title>"));
}

#[test]
fn singular_contribution() {
    assert!(render("days=1&from=").contains("5 contributions"));
    let mut one = calendar();
    one.days.last_mut().unwrap().1 = 1;
    let svg =
        flat(&render_graph(&one, today(), CardOptions::default(), &options("days=1")).unwrap());
    assert!(svg.contains("1 contribution ·"), "{svg}");
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
    let mut empty = calendar();
    empty.days.iter_mut().for_each(|(_, count)| *count = 0);
    let svg = flat(&render_graph(&empty, today(), CardOptions::default(), &options("")).unwrap());
    assert!(svg.contains("0 contributions"));
    assert_finite(&svg);
}

#[test]
fn out_of_window_requests_fail() {
    let graph = options("from=2026-11-10&to=2026-11-20");
    assert!(matches!(
        render_graph(&calendar(), today(), CardOptions::default(), &graph),
        Err(GraphError::OutsideWindow { .. })
    ));
}

#[test]
fn title_uses_the_name_then_the_login() {
    assert!(render("days=5").contains("The Octocat's Contribution Graph"));
    let mut anonymous = calendar();
    anonymous.name = None;
    let svg = flat(
        &render_graph(
            &anonymous,
            today(),
            CardOptions::default(),
            &options("days=5"),
        )
        .unwrap(),
    );
    assert!(svg.contains("octocat's Contribution Graph"), "{svg}");
}

#[test]
fn switches_change_the_output() {
    assert!(render("days=5&area=true").contains("url(#repobeats-area)"));
    assert!(!render("days=5").contains("url(#repobeats-area)"));
    assert!(!render("days=5&hide_title=true").contains("Octocat"));
    let straight = render("days=5&smooth=false");
    assert!(line_path(&straight).contains(" L") && !line_path(&straight).contains(" C"));
    let curved = render("days=5");
    assert!(line_path(&curved).contains(" C") && !line_path(&curved).contains(" L"));
    let static_svg = flat(
        &render_graph(
            &calendar(),
            today(),
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
            &calendar(),
            today(),
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
fn text_is_escaped_and_shortened() {
    let svg = render("days=5&custom_title=%3Cscript%3Ealert(1)%3C/script%3E%26%22x");
    assert!(!svg.contains("<script"));
    assert!(svg.contains("&lt;script&gt;"));
    let long = render(&format!("days=5&custom_title={}", "a".repeat(500)));
    assert!(!long.contains(&"a".repeat(100)));

    let mut hostile = calendar();
    hostile.name = Some("<img src=x onerror=alert(1)>".into());
    let svg = flat(
        &render_graph(
            &hostile,
            today(),
            CardOptions::default(),
            &options("days=5"),
        )
        .unwrap(),
    );
    assert!(!svg.contains("<img") && svg.contains("&lt;img"));
}

#[test]
fn error_card_escapes_the_message() {
    let svg = render_graph_error("bad <img src=x onerror=alert(1)> value");
    assert!(!svg.contains("<img"));
    assert!(svg.contains("&lt;img"));
}

fn graphql_body() -> serde_json::Value {
    json!({"user": {
        "login": "octocat",
        "name": "  The Octocat ",
        "contributionsCollection": {"contributionCalendar": {"weeks": [
            {"contributionDays": [
                {"date": "2026-11-08", "contributionCount": 0},
                {"date": "2026-11-07", "contributionCount": 4}
            ]},
            {"contributionDays": [
                {"date": "2026-11-09", "contributionCount": 12}
            ]}
        ]}}
    }})
}

#[test]
fn reads_the_graphql_calendar() {
    let calendar = ContributionCalendar::from_graphql(&graphql_body()).unwrap();
    assert_eq!(calendar.login, "octocat");
    assert_eq!(calendar.name.as_deref(), Some("The Octocat"));
    // Sorted oldest first even though the response was not.
    assert_eq!(
        calendar.days,
        vec![(day(11, 7), 4), (day(11, 8), 0), (day(11, 9), 12)]
    );
}

#[test]
fn unknown_users_and_odd_responses_give_none() {
    assert!(ContributionCalendar::from_graphql(&json!({"user": null})).is_none());
    assert!(ContributionCalendar::from_graphql(&json!({})).is_none());
    let mut bad = graphql_body();
    bad["user"]["contributionsCollection"]["contributionCalendar"]["weeks"][0]["contributionDays"]
        [0]["date"] = json!("yesterday");
    assert!(ContributionCalendar::from_graphql(&bad).is_none());
    let mut no_name = graphql_body();
    no_name["user"]["name"] = json!(null);
    assert_eq!(
        ContributionCalendar::from_graphql(&no_name).unwrap().name,
        None
    );
}

#[test]
fn usernames_are_validated() {
    assert_eq!(parse_username(" octocat "), Ok("octocat".to_owned()));
    assert_eq!(parse_username("a-b-1"), Ok("a-b-1".to_owned()));
    for bad in ["", "-a", "a-", "a b", "a/b", "a\"b", &"a".repeat(40)] {
        assert!(parse_username(bad).is_err(), "{bad:?}");
    }
}
