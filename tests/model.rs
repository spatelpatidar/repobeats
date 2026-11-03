use std::str::FromStr;

use chrono::{DateTime, Duration, TimeZone, Utc};
use repobeats::{
    domain::RepositoryName,
    model::{ACTIVITY_WINDOW_DAYS, ActivitySnapshot, EventKind, RepositoryEvent},
};

#[test]
fn aggregates_current_and_previous_windows() {
    let now = Utc.with_ymd_and_hms(2026, 8, 27, 12, 0, 0).unwrap();
    let mut events = Vec::new();

    for index in 0..47 {
        events.push(event(
            EventKind::Commit,
            now - Duration::days(index % 30),
            None,
            if index < 30 { "AprilNEA" } else { "3zet" },
        ));
    }
    for index in 0..19 {
        events.push(event(
            EventKind::Commit,
            now - Duration::days(30 + index),
            None,
            "previous",
        ));
    }
    for index in 0..5 {
        events.push(event(
            EventKind::Issue,
            now - Duration::days(index),
            (index == 0).then_some(now),
            "davidbudnick",
        ));
    }
    for index in 0..4 {
        events.push(event(
            EventKind::Issue,
            now - Duration::days(30 + index),
            (index == 0).then_some(now - Duration::days(30)),
            "previous",
        ));
    }

    let snapshot = ActivitySnapshot::from_events(repository(), now, &events);
    let period = snapshot.period(30).unwrap();

    assert_eq!(snapshot.current.len(), ACTIVITY_WINDOW_DAYS);
    assert_eq!(snapshot.previous.len(), ACTIVITY_WINDOW_DAYS);
    assert_eq!(
        period
            .current
            .iter()
            .map(|day| day.contributions())
            .sum::<u32>(),
        52
    );
    assert_eq!(ActivitySnapshot::pushes(period.current), 47);
    assert_eq!(ActivitySnapshot::pushes(period.previous), 19);
    assert_eq!(ActivitySnapshot::issue_ratio(period.current), 5.0);
    assert_eq!(ActivitySnapshot::issue_ratio(period.previous), 4.0);
    assert_eq!(ActivitySnapshot::pulls_opened(period.current), 0);
    assert_eq!(snapshot.contributors[0].name, "AprilNEA");
    assert_eq!(snapshot.contributors[0].total, 30);
}

#[test]
fn counts_closures_for_issues_created_before_collection_window() {
    let now = Utc.with_ymd_and_hms(2026, 8, 27, 12, 0, 0).unwrap();
    let old_issue = event(
        EventKind::Issue,
        now - Duration::days(800),
        Some(now - Duration::days(1)),
        "author",
    );

    let snapshot = ActivitySnapshot::from_events(repository(), now, &[old_issue]);

    assert_eq!(snapshot.current[ACTIVITY_WINDOW_DAYS - 2].issues_closed, 1);
    assert!(snapshot.contributors.is_empty());
}

#[test]
fn selects_immediately_preceding_periods_from_the_activity_window() {
    let now = Utc.with_ymd_and_hms(2026, 8, 27, 12, 0, 0).unwrap();
    let events = [
        event(EventKind::Commit, now, None, "current"),
        event(
            EventKind::Commit,
            now - Duration::days(90),
            None,
            "preceding",
        ),
    ];

    let snapshot = ActivitySnapshot::from_events(repository(), now, &events);
    let period = snapshot.period(90).unwrap();

    assert_eq!(ActivitySnapshot::pushes(period.current), 1);
    assert_eq!(ActivitySnapshot::pushes(period.previous), 1);
}

#[test]
fn supports_a_full_year_and_its_preceding_year() {
    let now = Utc.with_ymd_and_hms(2026, 8, 27, 12, 0, 0).unwrap();
    let events = [
        event(EventKind::Commit, now, None, "current"),
        event(
            EventKind::Commit,
            now - Duration::days(365),
            None,
            "previous",
        ),
    ];

    let snapshot = ActivitySnapshot::from_events(repository(), now, &events);
    let period = snapshot.period(365).unwrap();

    assert_eq!(ActivitySnapshot::pushes(period.current), 1);
    assert_eq!(ActivitySnapshot::pushes(period.previous), 1);
    assert_eq!(snapshot.contributors.len(), 1);
    assert_eq!(snapshot.contributors[0].name, "current");
}

#[test]
fn retains_all_contributors_for_period_specific_ranking() {
    let now = Utc.with_ymd_and_hms(2026, 8, 27, 12, 0, 0).unwrap();
    let events = (0..7)
        .map(|index| {
            event(
                EventKind::Commit,
                now - Duration::days(index),
                None,
                &format!("author-{index}"),
            )
        })
        .collect::<Vec<_>>();

    let snapshot = ActivitySnapshot::from_events(repository(), now, &events);

    assert_eq!(snapshot.contributors.len(), 7);
}

fn repository() -> RepositoryName {
    RepositoryName::from_str("owner/repo").unwrap()
}

fn event(
    kind: EventKind,
    created_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    contributor: &str,
) -> RepositoryEvent {
    RepositoryEvent {
        kind,
        created_at,
        closed_at,
        contributor: contributor.into(),
    }
}
