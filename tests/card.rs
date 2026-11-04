use std::str::FromStr;

use chrono::{TimeZone, Utc};
use repobeats::{
    card::{CardOptions, CardRenderer, CardSize, CardTheme, PeriodDays, SvgCardRenderer},
    domain::RepositoryName,
    model::{ActivitySnapshot, ContributorActivity, DailyActivity},
};

#[test]
fn renders_reference_values_and_safe_contributor_names() {
    let svg = SvgCardRenderer
        .render(&snapshot(), CardOptions::default())
        .unwrap();

    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("height=\"318\""));
    assert!(svg.contains("width=\"814\""));
    assert!(svg.contains("52 Contributions in the Last 30 Days"));
    assert!(svg.contains("5 Opened/Closed Issue Ratio"));
    assert!(svg.contains("0 Pull Requests Opened"));
    assert!(svg.contains("47 Pushes"));
    assert!(svg.contains("+1 (25%)"));
    assert!(svg.contains("+28 (147.37%)"));
    assert!(svg.contains("past month"));
    assert!(!svg.contains("previous period"));
    assert!(svg.contains("April&lt;NEA"));
    assert!(svg.contains("aprilnea[bot]"));
    assert_eq!(svg.matches("data-header-day=").count(), 30);
    assert!(svg.ends_with("</svg>"));
}

#[test]
fn supports_dark_small_seven_day_cards() {
    let options = CardOptions {
        theme: CardTheme::Dark,
        size: CardSize::Small,
        period: PeriodDays::SEVEN,
        ..CardOptions::default()
    };
    let svg = SvgCardRenderer.render(&snapshot(), options).unwrap();

    assert!(svg.contains("height=\"239\""));
    assert!(svg.contains("width=\"610\""));
    assert!(svg.contains("viewBox=\"0 0 814 318\""));
    assert!(svg.contains("fill=\"#0d1117\""));
    assert!(svg.contains("fill=\"#7ee787\""));
    assert!(svg.contains("fill=\"#56d364\""));
    assert!(svg.contains("52 Contributions in the Last 7 Days"));
    assert!(svg.contains("past 7 days"));
    assert_eq!(svg.matches("data-header-day=").count(), 7);
}

#[test]
fn rejects_unsupported_periods() {
    let error = serde_json::from_str::<PeriodDays>("8").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("period must be 7, 14, 30, 90, 180, or 365 days")
    );
}

#[test]
fn renders_ninety_day_half_year_and_year_cards() {
    for (period, days, label) in [
        (PeriodDays::NINETY, 90, "past 90 days"),
        (PeriodDays::ONE_EIGHTY, 180, "past 180 days"),
        (PeriodDays::YEAR, 365, "past year"),
    ] {
        let svg = SvgCardRenderer
            .render(
                &long_snapshot(),
                CardOptions {
                    period,
                    ..CardOptions::default()
                },
            )
            .unwrap();

        assert!(svg.contains(&format!("Contributions in the Last {days} Days")));
        assert!(svg.contains(label));
        assert_eq!(svg.matches("data-header-day=").count(), days);
        assert_eq!(svg.matches("height=\"5\"").count(), 150);
        for rectangle in svg
            .split("<rect")
            .filter_map(|node| node.split("/>").next())
        {
            if rectangle.contains("height=\"5\"") {
                assert!(rectangle.contains("y=\"298\"") || rectangle.contains("y=\"304\""));
            }
        }
    }
}

#[test]
fn old_snapshots_require_a_refresh_for_long_periods() {
    let error = SvgCardRenderer
        .render(
            &snapshot(),
            CardOptions {
                period: PeriodDays::NINETY,
                ..CardOptions::default()
            },
        )
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "snapshot does not contain enough history for a 90-day card"
    );
}

fn snapshot() -> ActivitySnapshot {
    let mut current = vec![DailyActivity::default(); 30];
    let mut previous = vec![DailyActivity::default(); 30];
    current[29].issues_opened = 5;
    current[29].issues_closed = 1;
    current[29].pushes = 47;
    previous[29].issues_opened = 4;
    previous[29].issues_closed = 1;
    previous[29].pushes = 19;
    ActivitySnapshot {
        repository: RepositoryName::from_str("owner/repo").unwrap(),
        generated_at: Utc.with_ymd_and_hms(2026, 8, 27, 12, 0, 0).unwrap(),
        current,
        previous,
        contributors: vec![
            ContributorActivity {
                name: "April<NEA".into(),
                total: 3,
                days: vec![1; 30],
            },
            ContributorActivity {
                name: "aprilnea[bot]".into(),
                total: 2,
                days: vec![0; 30],
            },
        ],
    }
}

fn long_snapshot() -> ActivitySnapshot {
    let mut current = vec![DailyActivity::default(); 365];
    let mut previous = vec![DailyActivity::default(); 365];
    for (index, day) in current.iter_mut().enumerate() {
        day.pushes = (index % 5) as u32;
        day.issues_opened = (index % 3 == 0) as u32;
        day.issues_closed = (index % 7 == 0) as u32;
    }
    for (index, day) in previous.iter_mut().enumerate() {
        day.pushes = (index % 3) as u32;
        day.issues_opened = (index % 5 == 0) as u32;
        day.issues_closed = (index % 11 == 0) as u32;
    }
    ActivitySnapshot {
        repository: RepositoryName::from_str("owner/repo").unwrap(),
        generated_at: Utc.with_ymd_and_hms(2026, 8, 27, 12, 0, 0).unwrap(),
        current,
        previous,
        contributors: (0..5)
            .map(|index| ContributorActivity {
                name: format!("contributor-{index}"),
                total: 365,
                days: (0..365)
                    .map(|day| ((day + index) % 4 == 0) as u32)
                    .collect(),
            })
            .collect(),
    }
}
