use std::str::FromStr;

use chrono::{TimeZone, Utc};
use repobeats::{
    card::{
        CardOptions, CardRenderer, CardTheme, ColorError, ColorScheme, SvgCardRenderer,
        render_contributors, render_languages, render_overview,
    },
    domain::RepositoryName,
    model::{ActivitySnapshot, ContributorActivity, DailyActivity, RepoOverview},
};
use serde_json::json;

const FIVE: &str = "#101820,#ff3366,#33ccff,#f2f2f2,#ffcc00";

fn repository() -> RepositoryName {
    RepositoryName::from_str("acme/widgets").unwrap()
}

fn snapshot() -> ActivitySnapshot {
    let mut current = vec![DailyActivity::default(); 30];
    let mut previous = vec![DailyActivity::default(); 30];
    for (index, day) in current.iter_mut().enumerate() {
        day.pushes = (index % 4) as u32;
        day.issues_opened = (index % 3 == 0) as u32;
        day.pulls_opened = (index % 5 == 0) as u32;
    }
    previous[29].pushes = 3;
    ActivitySnapshot {
        repository: repository(),
        generated_at: Utc.with_ymd_and_hms(2026, 8, 27, 12, 0, 0).unwrap(),
        current,
        previous,
        contributors: vec![ContributorActivity {
            name: "ada".into(),
            total: 30,
            days: vec![1; 30],
        }],
    }
}

fn with_colors(list: &str) -> CardOptions {
    CardOptions {
        colors: Some(list.parse().unwrap()),
        ..CardOptions::default()
    }
}

/// The `<text>` element that contains `needle`, so its `fill` can be checked.
fn text_element<'a>(svg: &'a str, needle: &str) -> &'a str {
    svg.split("<text")
        .find(|node| node.contains(needle))
        .unwrap_or_else(|| panic!("no text containing {needle}"))
}

#[test]
fn colors_map_by_position() {
    let svg = SvgCardRenderer
        .render(&snapshot(), with_colors(FIVE))
        .unwrap();

    // [0] background: the rectangle drawn behind everything
    assert!(svg.contains("<rect fill=\"#101820\"") || svg.contains("fill=\"#101820\""));
    // [1] primary: header title and the Issues series
    assert!(text_element(&svg, "Contributions in the Last").contains("fill=\"#ff3366\""));
    assert!(text_element(&svg, "Opened/Closed Issue Ratio").contains("fill=\"#ff3366\""));
    // [2] secondary: the Pull Requests series
    assert!(text_element(&svg, "Pull Requests Opened").contains("fill=\"#33ccff\""));
    // [4] tertiary: the Pushes series
    assert!(text_element(&svg, "Pushes</text>").contains("fill=\"#ffcc00\""));
    // [3] text: muted text is the text color mixed 60% into the background
    assert!(svg.contains("fill=\"#989b9e\""));
}

#[test]
fn comment_documents_the_positions() {
    let svg = SvgCardRenderer
        .render(&snapshot(), with_colors(FIVE))
        .unwrap();

    assert!(svg.contains(
        "<!-- repobeats colors: [0] background=#101820, [1] primary=#ff3366, \
         [2] secondary=#33ccff, [3] text=#f2f2f2, [4] tertiary=#ffcc00 -->"
    ));
}

#[test]
fn four_colors_derive_the_tertiary() {
    let scheme = ColorScheme::from_str("101820,ff0000,33ccff,f2f2f2").unwrap();
    assert!(scheme.tertiary.is_none());
    // hue of pure red turned 120 degrees is pure green
    assert_eq!(scheme.tertiary_or_derived().to_string(), "#00ff00");

    let svg = SvgCardRenderer
        .render(
            &snapshot(),
            CardOptions {
                colors: Some(scheme),
                ..CardOptions::default()
            },
        )
        .unwrap();
    assert!(svg.contains("[4] tertiary=#00ff00 (derived)"));
}

#[test]
fn transparent_background_is_only_allowed_first() {
    let scheme = ColorScheme::from_str("transparent,#ff3366,#33ccff,#222222").unwrap();
    assert!(scheme.background.is_none());
    let svg = SvgCardRenderer
        .render(
            &snapshot(),
            CardOptions {
                colors: Some(scheme),
                ..CardOptions::default()
            },
        )
        .unwrap();
    assert!(svg.contains("[0] background=transparent"));

    assert_eq!(
        ColorScheme::from_str("#000,transparent,#fff,#fff").unwrap_err(),
        ColorError::Transparent {
            index: 1,
            role: "primary"
        }
    );
}

#[test]
fn accepts_list_formats_and_rejects_bad_input() {
    let expected = ColorScheme::from_str(FIVE).unwrap();
    for input in [
        "101820, ff3366, 33ccff, f2f2f2, ffcc00",
        "[\"#101820\", \"#ff3366\", \"#33ccff\", \"#f2f2f2\", \"#ffcc00\"]",
        "#101820 #ff3366 #33ccff #f2f2f2 #ffcc00",
    ] {
        assert_eq!(ColorScheme::from_str(input).unwrap(), expected, "{input}");
    }
    assert_eq!(
        ColorScheme::from_str("#fff,#f00,#0f0,#000")
            .unwrap()
            .primary
            .to_string(),
        "#ff0000"
    );

    let count = ColorScheme::from_str("#000,#111,#222").unwrap_err();
    assert_eq!(count, ColorError::Count(3));
    assert!(count.to_string().contains("[0] background, [1] primary"));

    let bad = ColorScheme::from_str("#000,#12345,#222,#333").unwrap_err();
    assert_eq!(
        bad.to_string(),
        "colors[1] (primary) is `#12345`: use #rgb or #rrggbb"
    );
}

#[test]
fn colors_deserialize_from_a_json_array() {
    let options: CardOptions = serde_json::from_value(json!({
        "colors": ["#101820", "#ff3366", "#33ccff", "#f2f2f2"]
    }))
    .unwrap();
    assert_eq!(options.colors.unwrap().primary.to_string(), "#ff3366");
    assert!(options.animate);

    let error = serde_json::from_value::<CardOptions>(json!({"colors": ["#fff"]})).unwrap_err();
    assert!(error.to_string().contains("needs 4 or 5 values"));
}

#[test]
fn colors_replace_the_theme_on_every_card() {
    let overview = RepoOverview::from_json(
        repository(),
        Utc::now(),
        &json!({"stargazers_count": 3}),
        None,
    );
    let options = CardOptions {
        theme: CardTheme::Dracula,
        ..with_colors(FIVE)
    };
    let cards = [
        SvgCardRenderer.render(&snapshot(), options).unwrap(),
        render_contributors(&snapshot(), options).unwrap(),
        render_languages(&repository(), &[("Rust".into(), 10)], options),
        render_overview(&overview, options),
    ];
    // [3] text: legend names on the languages card, values on the overview card
    assert!(cards[2].contains("fill=\"#f2f2f2\""));
    assert!(cards[3].contains("fill=\"#f2f2f2\""));
    for svg in cards {
        assert!(svg.contains("fill=\"#101820\""));
        assert!(svg.contains("fill=\"#ff3366\""));
        assert!(svg.contains("[1] primary=#ff3366"));
        assert!(!svg.contains("#24292f"), "built-in text color leaked");
        assert!(!svg.contains("#282a36"), "dracula background leaked");
    }
}

#[test]
fn animation_is_on_by_default_and_can_be_turned_off() {
    let animated = SvgCardRenderer
        .render(&snapshot(), CardOptions::default())
        .unwrap();
    for expected in [
        "@keyframes rise",
        "@keyframes grow",
        "@keyframes ping",
        "prefers-reduced-motion",
        "class=\"bar w",
        "class=\"cell w",
        "class=\"ping\"",
    ] {
        assert!(animated.contains(expected), "missing {expected}");
    }

    let still = SvgCardRenderer
        .render(
            &snapshot(),
            CardOptions {
                animate: false,
                ..CardOptions::default()
            },
        )
        .unwrap();
    assert!(!still.contains("@keyframes"));
    assert!(!still.contains("prefers-reduced-motion"));
}

#[test]
fn every_card_animates_and_stays_well_formed() {
    let overview = RepoOverview::from_json(
        repository(),
        Utc::now(),
        &json!({"stargazers_count": 3}),
        None,
    );
    let options = CardOptions::default();
    let cards = [
        SvgCardRenderer.render(&snapshot(), options).unwrap(),
        render_contributors(&snapshot(), options).unwrap(),
        render_languages(&repository(), &[("Rust".into(), 10)], options),
        render_overview(&overview, options),
    ];
    for svg in cards {
        assert!(svg.contains("@keyframes rise"));
        assert_eq!(
            svg.matches("<g ").count() + svg.matches("<g>").count(),
            svg.matches("</g>").count(),
            "unbalanced groups"
        );
    }
}
