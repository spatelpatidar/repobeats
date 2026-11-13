//! HTTP server: `GET /graph` returns the contribution graph as SVG, `GET /`
//! serves a page that builds the URL from the parameters.
//!
//! ```text
//! REPOBEATS_TOKEN=ghp_... cargo run --features server --bin repobeats-server
//! curl 'http://localhost:5100/graph?user=spatelpatidar&repo=repobeats&days=31&area=true'
//! ```
//!
//! Environment: `PORT` (default 5100), `REPOBEATS_TOKEN` or `GITHUB_TOKEN`
//! (strongly recommended: anonymous access allows 60 GitHub requests an hour),
//! `GITHUB_API_URL`, `REPOBEATS_CACHE_SECONDS` (default 1800).

use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use axum::{
    Router,
    extract::{RawQuery, State},
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use chrono::Utc;
use repobeats::{
    card::{CardOptions, CardSize, CardTheme, GraphOptions, render_graph, render_graph_error},
    collector::Collector,
    domain::RepositoryName,
    model::ActivitySnapshot,
    params::{ParamError, invalid, parse_bool, parse_enum},
};

const PAGE: &str = include_str!("../web/index.html");
const MAX_CACHE_ENTRIES: usize = 500;
/// History is fetched in these sizes so `days=7` and `days=31` share one fetch.
const LOOKBACK_BUCKETS: [i64; 4] = [31, 90, 180, 365];

/// Snapshots by (lowercased repository, lookback days), with the time fetched.
type Cache = HashMap<(String, i64), (Instant, Arc<ActivitySnapshot>)>;

struct App {
    collector: Collector,
    ttl: Duration,
    cache: Mutex<Cache>,
}

impl App {
    async fn snapshot(
        &self,
        repository: &RepositoryName,
        lookback: i64,
    ) -> Result<Arc<ActivitySnapshot>> {
        let bucket = LOOKBACK_BUCKETS
            .into_iter()
            .find(|bucket| *bucket >= lookback)
            .unwrap_or(365);
        let key = (repository.to_string().to_ascii_lowercase(), bucket);
        if let Some((at, snapshot)) = self.cache.lock().unwrap().get(&key)
            && at.elapsed() < self.ttl
        {
            return Ok(Arc::clone(snapshot));
        }
        let snapshot = Arc::new(self.collector.collect_days(repository, bucket).await?);
        let mut cache = self.cache.lock().unwrap();
        if cache.len() >= MAX_CACHE_ENTRIES {
            let ttl = self.ttl;
            cache.retain(|_, (at, _)| at.elapsed() < ttl);
            if cache.len() >= MAX_CACHE_ENTRIES {
                cache.clear();
            }
        }
        cache.insert(key, (Instant::now(), Arc::clone(&snapshot)));
        Ok(snapshot)
    }
}

/// Everything in the query string, split into the repository, the card
/// options shared with the other cards, and the graph options.
struct Request {
    repository: RepositoryName,
    card: CardOptions,
    graph: GraphOptions,
}

fn parse_request(query: &str) -> Result<Request, ParamError> {
    let pairs = form_urlencoded::parse(query.as_bytes())
        .map(|(key, value)| (key.to_ascii_lowercase(), value.trim().to_owned()))
        .filter(|(_, value)| !value.is_empty())
        .collect::<Vec<_>>();
    let get = |name: &str| {
        pairs
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };

    let repo = get("repo").ok_or(ParamError::Missing("repo"))?;
    let repository = if repo.contains('/') {
        repo.parse()
    } else {
        let user = get("user")
            .or_else(|| get("username"))
            .ok_or(ParamError::Missing("user"))?;
        RepositoryName::new(user, repo)
    }
    .map_err(|_| invalid("repo", repo, "use a name with `user`, or owner/name"))?;

    let mut card = CardOptions::default();
    if let Some(theme) = get("theme") {
        card.theme = parse_enum::<CardTheme>(
            "theme",
            theme,
            "use light, dark, github-dimmed, dracula, nord, tokyo-night or solarized-light",
        )?;
    }
    if let Some(size) = get("size") {
        card.size = parse_enum::<CardSize>("size", size, "use small, medium or large")?;
    }
    if let Some(animate) = get("animate") {
        card.animate = parse_bool("animate", animate)?;
    }
    Ok(Request {
        repository,
        card,
        graph: GraphOptions::from_pairs(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())))?,
    })
}

fn svg(status: StatusCode, cache: &'static str, body: String) -> Response {
    (
        status,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("image/svg+xml; charset=utf-8"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static(cache)),
            // The card is meant to be embedded from other origins (READMEs, sites).
            (
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            ),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        body,
    )
        .into_response()
}

fn failure(status: StatusCode, message: &str) -> Response {
    svg(status, "no-store", render_graph_error(message))
}

async fn graph(State(app): State<Arc<App>>, RawQuery(query): RawQuery) -> Response {
    let request = match parse_request(&query.unwrap_or_default()) {
        Ok(request) => request,
        Err(error) => return failure(StatusCode::BAD_REQUEST, &error.to_string()),
    };
    let lookback = request.graph.lookback_days(Utc::now().date_naive());
    let snapshot = match app.snapshot(&request.repository, lookback).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let message = format!("{error}");
            let status = if message.contains("404") {
                StatusCode::NOT_FOUND
            } else if message.contains("403") {
                StatusCode::TOO_MANY_REQUESTS
            } else {
                StatusCode::BAD_GATEWAY
            };
            eprintln!("repobeats-server: {}: {error:#}", request.repository);
            return failure(status, &message);
        }
    };
    match render_graph(&snapshot, request.card, &request.graph) {
        Ok(body) => svg(StatusCode::OK, "public, max-age=1800", body),
        Err(error) => failure(StatusCode::BAD_REQUEST, &error.to_string()),
    }
}

async fn page() -> Html<&'static str> {
    Html(PAGE)
}

async fn health() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() -> Result<()> {
    let token = ["REPOBEATS_TOKEN", "GITHUB_TOKEN"]
        .iter()
        .find_map(|name| env::var(name).ok().filter(|value| !value.trim().is_empty()));
    if token.is_none() {
        eprintln!("repobeats-server: no token set, GitHub allows 60 requests/hour anonymously");
    }
    let api_url = env::var("GITHUB_API_URL").ok();
    let ttl = env::var("REPOBEATS_CACHE_SECONDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_800);
    let app = Arc::new(App {
        collector: Collector::new(token.as_deref(), api_url.as_deref(), 50)?,
        ttl: Duration::from_secs(ttl),
        cache: Mutex::new(HashMap::new()),
    });
    let router = Router::new()
        .route("/", get(page))
        .route("/graph", get(graph))
        .route("/healthz", get(health))
        .with_state(app);

    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(5_100_u16);
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("cannot listen on {address}"))?;
    eprintln!("repobeats-server: listening on http://{address}");
    axum::serve(listener, router).await?;
    Ok(())
}
