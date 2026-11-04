//! Collects repository activity with a plain access token.
//!
//! The token is whatever the caller supplies: the workflow's built-in
//! `GITHUB_TOKEN`, a personal access token, or nothing at all for public
//! repositories (anonymous requests, lower rate limit). No OAuth flow, GitHub
//! App or server is involved, and nothing is sent anywhere except api.github.com.

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration, Utc};
use octocrab::{
    Octocrab, Page,
    models::{issues::Issue, repos::RepoCommit},
    params::{Direction, State, issues::Sort},
};
use serde::de::DeserializeOwned;

use serde_json::Value;

use crate::{
    domain::RepositoryName,
    model::{
        ActivitySnapshot, COLLECTION_DAYS, EventKind, LanguageBreakdown, RepoOverview,
        RepositoryEvent, parse_languages,
    },
};

pub struct Collector {
    client: Octocrab,
    max_pages: u32,
}

impl Collector {
    /// `token` may be `None` or empty for public repositories.
    ///
    /// `api_url` overrides https://api.github.com (GitHub Enterprise Server).
    pub fn new(token: Option<&str>, api_url: Option<&str>, max_pages: u32) -> Result<Self> {
        let mut builder = Octocrab::builder();
        if let Some(url) = api_url.map(str::trim).filter(|url| !url.is_empty()) {
            builder = builder.base_uri(url).context("invalid GITHUB_API_URL")?;
        }
        if let Some(token) = token.map(str::trim).filter(|token| !token.is_empty()) {
            builder = builder.personal_token(token.to_owned());
        }
        Ok(Self {
            client: builder.build().context("failed to build GitHub client")?,
            max_pages,
        })
    }

    pub async fn collect(&self, repository: &RepositoryName) -> Result<ActivitySnapshot> {
        let now = Utc::now();
        let since = now - Duration::days(COLLECTION_DAYS);
        let (commits, issues) = tokio::try_join!(
            self.commits(repository, since, now),
            self.issues(repository, since),
        )?;
        let mut events = Vec::with_capacity(commits.len() + issues.len());

        for commit in commits {
            let created_at = commit
                .commit
                .author
                .as_ref()
                .and_then(|author| author.date)
                .or_else(|| {
                    commit
                        .commit
                        .committer
                        .as_ref()
                        .and_then(|committer| committer.date)
                });
            let Some(created_at) = created_at else {
                continue;
            };
            let contributor = commit
                .author
                .as_ref()
                .map(|author| author.login.clone())
                .or_else(|| {
                    commit
                        .commit
                        .author
                        .as_ref()
                        .map(|author| author.name.clone())
                })
                .unwrap_or_else(|| "unknown".to_owned());
            events.push(RepositoryEvent {
                kind: EventKind::Commit,
                created_at,
                closed_at: None,
                contributor,
            });
        }

        for issue in issues {
            events.push(RepositoryEvent {
                kind: if issue.pull_request.is_some() {
                    EventKind::PullRequest
                } else {
                    EventKind::Issue
                },
                created_at: issue.created_at,
                closed_at: issue.closed_at,
                contributor: issue.user.login,
            });
        }

        Ok(ActivitySnapshot::from_events(
            repository.clone(),
            now,
            &events,
        ))
    }

    /// One request: bytes of code per language.
    pub async fn languages(&self, repository: &RepositoryName) -> Result<LanguageBreakdown> {
        let route = format!("/repos/{repository}/languages");
        let body = self.get_json(&route, "languages").await?;
        Ok(parse_languages(&body))
    }

    /// Two requests: repository details and the latest release (none is fine).
    pub async fn overview(&self, repository: &RepositoryName) -> Result<RepoOverview> {
        let now = Utc::now();
        let repo = self
            .get_json(&format!("/repos/{repository}"), "repository")
            .await?;
        let release = match self
            .client
            .get::<Value, _, ()>(format!("/repos/{repository}/releases/latest"), None::<&()>)
            .await
        {
            Ok(body) => body["tag_name"]
                .as_str()
                .or_else(|| body["name"].as_str())
                .map(str::to_owned),
            Err(error) if status(&error) == Some(404) => None,
            Err(error) => {
                let hint = request_hint("latest release", &error);
                return Err(anyhow::Error::new(error).context(hint));
            }
        };
        Ok(RepoOverview::from_json(
            repository.clone(),
            now,
            &repo,
            release,
        ))
    }

    async fn get_json(&self, route: &str, what: &str) -> Result<Value> {
        self.client
            .get::<Value, _, ()>(route, None::<&()>)
            .await
            .map_err(|error| {
                let hint = request_hint(what, &error);
                anyhow::Error::new(error).context(hint)
            })
    }

    async fn commits(
        &self,
        repository: &RepositoryName,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<RepoCommit>> {
        let first = self
            .client
            .repos(repository.owner(), repository.name())
            .list_commits()
            .since(since)
            .until(until)
            .per_page(100_u8)
            .send()
            .await;
        let page = match first {
            Ok(page) => page,
            // 409 means the repository has no commits yet.
            Err(error) if status(&error) == Some(409) => return Ok(Vec::new()),
            Err(error) => {
                let hint = request_hint("commits", &error);
                return Err(anyhow::Error::new(error).context(hint));
            }
        };
        self.collect_pages(page).await
    }

    async fn issues(
        &self,
        repository: &RepositoryName,
        since: DateTime<Utc>,
    ) -> Result<Vec<Issue>> {
        let page = self
            .client
            .issues(repository.owner(), repository.name())
            .list()
            .state(State::All)
            .since(since)
            .sort(Sort::Updated)
            .direction(Direction::Descending)
            .per_page(100_u8)
            .send()
            .await
            .map_err(|error| {
                let hint = request_hint("issues", &error);
                anyhow::Error::new(error).context(hint)
            })?;
        self.collect_pages(page).await
    }

    async fn collect_pages<T>(&self, mut page: Page<T>) -> Result<Vec<T>>
    where
        T: DeserializeOwned,
    {
        let mut items = Vec::new();
        for page_number in 1..=self.max_pages {
            items.append(&mut page.items);
            if page.next.is_none() {
                return Ok(items);
            }
            if page_number == self.max_pages {
                bail!(
                    "pagination exceeded max_pages ({}); raise `max_pages` to include more history",
                    self.max_pages
                );
            }
            page = self
                .client
                .get_page(&page.next)
                .await
                .context("GitHub pagination request failed")?
                .context("GitHub pagination omitted the next page")?;
        }
        Ok(items)
    }
}

fn status(error: &octocrab::Error) -> Option<u16> {
    match error {
        octocrab::Error::GitHub { source, .. } => Some(source.status_code.as_u16()),
        _ => None,
    }
}

fn request_hint(what: &str, error: &octocrab::Error) -> String {
    match status(error) {
        Some(401) => format!(
            "GitHub rejected the token while reading {what} (401): it is invalid or expired"
        ),
        Some(403) => format!(
            "GitHub refused the {what} request (403): rate limit hit, or the token lacks read access"
        ),
        Some(404) => format!(
            "GitHub returned 404 for {what}: the repository does not exist, or the token cannot see it (private repositories need a token with read access to contents, issues and pull requests)"
        ),
        _ => format!("GitHub {what} request failed"),
    }
}
