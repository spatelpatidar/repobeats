//! Renders a sample profile graph from made-up data:
//! `cargo run --example graph_preview -- "days=31&area=true" dark > out.svg`
use chrono::{Duration, NaiveDate};
use repobeats::{
    card::{CardOptions, CardTheme, GraphOptions, render_graph},
    model::ContributionCalendar,
};

fn main() {
    let query = std::env::args().nth(1).unwrap_or_default();
    let theme = std::env::args().nth(2).unwrap_or_else(|| "light".into());
    let today = NaiveDate::from_ymd_opt(2026, 11, 13).unwrap();
    let days = (0..365u32)
        .map(|back| {
            (
                today - Duration::days(364 - i64::from(back)),
                (back * 7 % 11) % 6,
            )
        })
        .collect();
    let calendar = ContributionCalendar {
        login: "octocat".into(),
        name: Some("The Octocat".into()),
        days,
    };
    let card = CardOptions {
        theme: serde_json::from_value(serde_json::json!(theme)).unwrap_or(CardTheme::Light),
        ..CardOptions::default()
    };
    let graph = GraphOptions::from_query(&query).expect("bad query");
    print!(
        "{}",
        render_graph(&calendar, today, card, &graph).expect("render")
    );
}
