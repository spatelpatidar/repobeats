use std::collections::HashMap;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::RepositoryName;

pub const ACTIVITY_WINDOW_DAYS: usize = 365;
pub const COLLECTION_DAYS: i64 = (ACTIVITY_WINDOW_DAYS * 2) as i64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventKind {
    Commit,
    Issue,
    PullRequest,
}

#[derive(Clone, Debug)]
pub struct RepositoryEvent {
    pub kind: EventKind,
    pub created_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub contributor: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DailyActivity {
    pub issues_opened: u32,
    pub issues_closed: u32,
    pub pulls_opened: u32,
    pub pulls_closed: u32,
    pub pushes: u32,
}

impl DailyActivity {
    pub fn contributions(&self) -> u32 {
        self.issues_opened + self.pulls_opened + self.pushes
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ContributorActivity {
    pub name: String,
    pub total: u32,
    pub days: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActivitySnapshot {
    pub repository: RepositoryName,
    pub generated_at: DateTime<Utc>,
    pub current: Vec<DailyActivity>,
    pub previous: Vec<DailyActivity>,
    pub contributors: Vec<ContributorActivity>,
}

#[derive(Clone, Copy, Debug)]
pub struct ActivityPeriod<'a> {
    pub current: &'a [DailyActivity],
    pub previous: &'a [DailyActivity],
}

impl ActivitySnapshot {
    pub fn from_events(
        repository: RepositoryName,
        generated_at: DateTime<Utc>,
        events: &[RepositoryEvent],
    ) -> Self {
        let current_start =
            generated_at.date_naive() - Duration::days((ACTIVITY_WINDOW_DAYS - 1) as i64);
        let previous_start = current_start - Duration::days(ACTIVITY_WINDOW_DAYS as i64);
        let mut current = vec![DailyActivity::default(); ACTIVITY_WINDOW_DAYS];
        let mut previous = vec![DailyActivity::default(); ACTIVITY_WINDOW_DAYS];
        let mut contributors: HashMap<String, Vec<u32>> = HashMap::new();

        for event in events {
            let created = event.created_at.date_naive();
            if let Some(index) = day_index(created, current_start) {
                add_created(&mut current[index], event.kind);
                let name = normalized_contributor(&event.contributor);
                contributors.entry(name).or_insert_with(empty_days)[index] += 1;
            } else if let Some(index) = day_index(created, previous_start) {
                add_created(&mut previous[index], event.kind);
            }

            if let Some(closed_at) = event.closed_at {
                let closed = closed_at.date_naive();
                if let Some(index) = day_index(closed, current_start) {
                    add_closed(&mut current[index], event.kind);
                } else if let Some(index) = day_index(closed, previous_start) {
                    add_closed(&mut previous[index], event.kind);
                }
            }
        }

        let mut contributors = contributors
            .into_iter()
            .map(|(name, days)| ContributorActivity {
                total: days.iter().sum(),
                name,
                days,
            })
            .collect::<Vec<_>>();
        contributors.sort_by(|left, right| {
            right
                .total
                .cmp(&left.total)
                .then_with(|| left.name.cmp(&right.name))
        });
        Self {
            repository,
            generated_at,
            current,
            previous,
            contributors,
        }
    }

    pub fn period(&self, days: usize) -> Option<ActivityPeriod<'_>> {
        if days == 0 || self.current.len() < days {
            return None;
        }
        let current_start = self.current.len() - days;
        let previous = if current_start >= days {
            &self.current[current_start - days..current_start]
        } else if current_start == 0 && self.previous.len() >= days {
            &self.previous[self.previous.len() - days..]
        } else {
            return None;
        };
        Some(ActivityPeriod {
            current: &self.current[current_start..],
            previous,
        })
    }

    pub fn pushes(days: &[DailyActivity]) -> u32 {
        days.iter().map(|day| day.pushes).sum()
    }

    pub fn pulls_opened(days: &[DailyActivity]) -> u32 {
        days.iter().map(|day| day.pulls_opened).sum()
    }

    pub fn issue_ratio(days: &[DailyActivity]) -> f64 {
        let opened: u32 = days.iter().map(|day| day.issues_opened).sum();
        let closed: u32 = days.iter().map(|day| day.issues_closed).sum();
        if closed == 0 {
            f64::from(opened)
        } else {
            f64::from(opened) / f64::from(closed)
        }
    }
}

fn empty_days() -> Vec<u32> {
    vec![0; ACTIVITY_WINDOW_DAYS]
}

fn day_index(day: NaiveDate, start: NaiveDate) -> Option<usize> {
    let index = day.signed_duration_since(start).num_days();
    (0..ACTIVITY_WINDOW_DAYS as i64)
        .contains(&index)
        .then_some(index as usize)
}

fn normalized_contributor(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        "unknown".to_owned()
    } else {
        name.to_owned()
    }
}

fn add_created(day: &mut DailyActivity, kind: EventKind) {
    match kind {
        EventKind::Commit => day.pushes += 1,
        EventKind::Issue => day.issues_opened += 1,
        EventKind::PullRequest => day.pulls_opened += 1,
    }
}

fn add_closed(day: &mut DailyActivity, kind: EventKind) {
    match kind {
        EventKind::Commit => {}
        EventKind::Issue => day.issues_closed += 1,
        EventKind::PullRequest => day.pulls_closed += 1,
    }
}
