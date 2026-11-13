use std::{env, fs, path::PathBuf, process::ExitCode};

use anyhow::{Context, Result, bail};
use repobeats::{
    card::{
        CardKind, CardOptions, CardRenderer, CardSize, CardTheme, ColorScheme, GraphOptions,
        PeriodDays, SvgCardRenderer, render_contributors, render_graph, render_languages,
        render_overview,
    },
    collector::Collector,
    domain::RepositoryName,
};

const USAGE: &str = "\
repobeats: write a GitHub repository activity card to an SVG file.

Usage: repobeats [--repo OWNER/NAME] [--user OWNER]
                 [--card activity|languages|contributors|overview]
                 [--theme light|dark|github-dimmed|dracula|nord|tokyo-night|solarized-light]
                 [--size small|medium|large] [--period 7|14|30|90|180|365]
                 [--colors \"#bg,#primary,#secondary,#text[,#tertiary]\"]
                 [--animate true|false]
                 [--graph-options \"days=31&area=true&line=58a6ff\"]
                 [--filename FILE.svg] [--max-pages N]

Token (optional for public repositories) is read from REPOBEATS_TOKEN,
then GITHUB_TOKEN. If --repo is omitted, GITHUB_REPOSITORY is used.
Every option can also be set with a REPOBEATS_<NAME> environment variable.

--colors takes 4 or 5 hex codes. The position decides the role and replaces --theme:
  [0] background   card background (or `transparent`); panels and borders derive from it
  [1] primary      titles, header icon and strip, Issues series, contributor names and bars
  [2] secondary    Pull Requests series
  [3] text         values and labels; muted text derives from it
  [4] tertiary     Pushes series (optional, derived from primary when omitted)
Example: --colors \"#0d1117,#58a6ff,#bc8cff,#e6edf3,#f78166\"
--animate false writes a static card (animations are on by default).

--card graph draws a contribution line graph. Its extra options go in
--graph-options as a URL query string (days, from, to, metric, area, grid,
smooth, hide_title, custom_title, hide_border, radius, height, bg_color,
border_color, color, title_color, line, point, area_color).";

#[derive(Default)]
struct Args {
    repo: Option<String>,
    user: Option<String>,
    card: Option<String>,
    theme: Option<String>,
    size: Option<String>,
    period: Option<String>,
    colors: Option<String>,
    animate: Option<String>,
    graph_options: Option<String>,
    filename: Option<String>,
    max_pages: Option<String>,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut args = Self::default();
        let mut iter = env::args().skip(1);
        while let Some(flag) = iter.next() {
            if matches!(flag.as_str(), "-h" | "--help") {
                println!("{USAGE}");
                std::process::exit(0);
            }
            let slot = match flag.as_str() {
                "--repo" => &mut args.repo,
                "--user" => &mut args.user,
                "--card" => &mut args.card,
                "--theme" => &mut args.theme,
                "--size" => &mut args.size,
                "--period" => &mut args.period,
                "--colors" => &mut args.colors,
                "--animate" => &mut args.animate,
                "--graph-options" => &mut args.graph_options,
                "--filename" => &mut args.filename,
                "--max-pages" => &mut args.max_pages,
                other => bail!("unknown argument `{other}`\n\n{USAGE}"),
            };
            *slot = Some(
                iter.next()
                    .with_context(|| format!("{flag} needs a value"))?,
            );
        }
        Ok(args)
    }
}

/// Value from a flag, else REPOBEATS_<NAME>; empty strings count as unset
/// (GitHub Actions passes unset inputs as empty strings).
fn pick(flag: Option<String>, name: &str) -> Option<String> {
    flag.or_else(|| env::var(format!("REPOBEATS_{name}")).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn repository(args: &mut Args) -> Result<RepositoryName> {
    let repo = pick(args.repo.take(), "REPO")
        .or_else(|| env::var("GITHUB_REPOSITORY").ok().filter(|v| !v.is_empty()))
        .context("no repository: set `repo` (OWNER/NAME, or a bare name together with `user`)")?;
    if repo.contains('/') {
        return Ok(repo.parse()?);
    }
    let user = pick(args.user.take(), "USER")
        .or_else(|| {
            env::var("GITHUB_REPOSITORY_OWNER")
                .ok()
                .filter(|v| !v.is_empty())
        })
        .context("`repo` has no owner: set `user` or use OWNER/NAME")?;
    Ok(RepositoryName::new(user, repo)?)
}

fn enum_value<T: serde::de::DeserializeOwned + Default>(
    value: Option<String>,
    what: &str,
) -> Result<T> {
    match value {
        None => Ok(T::default()),
        Some(value) => serde_json::from_value(serde_json::Value::String(value.clone()))
            .with_context(|| format!("invalid {what} `{value}`")),
    }
}

fn boolean(value: Option<String>, what: &str, default: bool) -> Result<bool> {
    match value.as_deref().map(str::to_ascii_lowercase).as_deref() {
        None => Ok(default),
        Some("true" | "on" | "yes" | "1") => Ok(true),
        Some("false" | "off" | "no" | "0") => Ok(false),
        Some(other) => bail!("invalid {what} `{other}` (use true or false)"),
    }
}

async fn run() -> Result<()> {
    let mut args = Args::parse()?;
    let repository = repository(&mut args)?;
    let options = CardOptions {
        kind: enum_value::<CardKind>(
            pick(args.card.take(), "CARD"),
            "card (activity, languages, contributors, overview or graph)",
        )?,
        theme: enum_value::<CardTheme>(
            pick(args.theme.take(), "THEME"),
            "theme (light, dark, github-dimmed, dracula, nord, tokyo-night or solarized-light)",
        )?,
        size: enum_value::<CardSize>(
            pick(args.size.take(), "SIZE"),
            "size (small, medium or large)",
        )?,
        period: match pick(args.period.take(), "PERIOD") {
            None => PeriodDays::default(),
            Some(days) => {
                PeriodDays::try_from(days.parse::<u16>().context("period must be a number")?)?
            }
        },
        colors: pick(args.colors.take(), "COLORS")
            .map(|list| list.parse::<ColorScheme>())
            .transpose()?,
        animate: boolean(pick(args.animate.take(), "ANIMATE"), "animate", true)?,
    };
    // Parsed before any network call, so a typo fails fast.
    let graph = (options.kind == CardKind::Graph)
        .then(|| {
            GraphOptions::from_query(
                &pick(args.graph_options.take(), "GRAPH_OPTIONS").unwrap_or_default(),
            )
        })
        .transpose()?;
    let filename = PathBuf::from(
        pick(args.filename.take(), "FILENAME").unwrap_or_else(|| "repobeats.svg".to_owned()),
    );
    let max_pages = match pick(args.max_pages.take(), "MAX_PAGES") {
        None => 50,
        Some(pages) => pages.parse().context("max_pages must be a number")?,
    };
    let token = ["REPOBEATS_TOKEN", "GITHUB_TOKEN"]
        .iter()
        .find_map(|name| env::var(name).ok().filter(|value| !value.trim().is_empty()));
    if token.is_none() {
        eprintln!(
            "repobeats: no token set, using anonymous access (public repositories only, 60 requests/hour)"
        );
    }

    let api_url = env::var("GITHUB_API_URL").ok();

    eprintln!(
        "repobeats: collecting {repository} ({:?} card)",
        options.kind
    );
    let collector = Collector::new(token.as_deref(), api_url.as_deref(), max_pages)?;
    let svg = match options.kind {
        CardKind::Activity => {
            let snapshot = collector.collect(&repository).await?;
            SvgCardRenderer.render(&snapshot, options)?
        }
        CardKind::Contributors => {
            let snapshot = collector.collect(&repository).await?;
            render_contributors(&snapshot, options)?
        }
        CardKind::Languages => {
            let languages = collector.languages(&repository).await?;
            render_languages(&repository, &languages, options)
        }
        CardKind::Overview => {
            let overview = collector.overview(&repository).await?;
            render_overview(&overview, options)
        }
        CardKind::Graph => {
            let graph = graph.context("graph options were not parsed")?;
            let today = chrono::Utc::now().date_naive();
            let snapshot = collector
                .collect_days(&repository, graph.lookback_days(today))
                .await?;
            render_graph(&snapshot, options, &graph)?
        }
    };

    if let Some(parent) = filename
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&filename, svg).with_context(|| format!("failed to write {}", filename.display()))?;
    eprintln!("repobeats: wrote {}", filename.display());
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("repobeats: error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
