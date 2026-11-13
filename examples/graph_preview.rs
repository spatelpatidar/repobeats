//! Renders a sample graph from synthetic data: `cargo run --example graph_preview -- "days=31&area=true" > out.svg`
use chrono::{TimeZone, Utc};
use repobeats::{
    card::{CardOptions, CardTheme, GraphOptions, render_graph},
    domain::RepositoryName,
    model::{ActivitySnapshot, DailyActivity},
};

fn main() {
    let query = std::env::args().nth(1).unwrap_or_default();
    let theme = std::env::args().nth(2).unwrap_or_default();
    let current = (0..365u32)
        .map(|i| DailyActivity {
            pushes: (i * 7 % 11) % 6,
            issues_opened: (i % 5 == 0) as u32,
            pulls_opened: (i % 9 == 0) as u32 * 2,
            ..DailyActivity::default()
        })
        .collect();
    let snapshot = ActivitySnapshot {
        repository: "acme/widgets".parse::<RepositoryName>().unwrap(),
        generated_at: Utc.with_ymd_and_hms(2026, 11, 13, 12, 0, 0).unwrap(),
        current,
        previous: vec![DailyActivity::default(); 365],
        contributors: vec![],
    };
    let card = CardOptions {
        theme: serde_json::from_value(serde_json::json!(if theme.is_empty() {
            "light"
        } else {
            &theme
        }))
        .unwrap_or(CardTheme::Light),
        ..CardOptions::default()
    };
    let graph = GraphOptions::from_query(&query).expect("bad query");
    print!("{}", render_graph(&snapshot, card, &graph).expect("render"));
}
