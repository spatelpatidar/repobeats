# Repobeats

GitHub repository cards (activity, languages, contributors, overview), generated as SVG files by a GitHub Action in your own workflow.

There is no sign-in, no GitHub App to install, no hosted service and no database. The Action runs on your runner as a prebuilt Docker image, reads the repository with a token you provide, and writes an SVG file. Your token and your repository data never go to a third party; the only network calls are to `api.github.com`.

The card shows pushes, opened and closed issues, opened and closed pull requests, the five most active contributors, and the change against the previous period of the same length.

## Examples

Generated weekly by [`showcase.yml`](.github/workflows/showcase.yml) and stored on the `output` branch.

**`card: activity`** pushes, issues, pull requests, top contributors, change against the previous period

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/activity-dark.svg">
  <img alt="Activity card" src="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/activity-light.svg">
</picture>

**`card: contributors`** ranked contributors with a bar per person

<img alt="Contributors card" src="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/contributors-dark.svg">

**`card: languages`** language breakdown by bytes of code

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/languages-dracula.svg">
  <img alt="Languages card" src="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/languages-light.svg">
</picture>

**`card: overview`** stars, forks, open issues, watchers, license, last push, latest release

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/overview-nord.svg">
  <img alt="Overview card" src="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/overview-light.svg">
</picture>

**Themes** (`theme:`), shown on the activity card

<img alt="light theme" width="400" src="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/activity-light.svg">
<img alt="dark theme" width="400" src="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/activity-dark.svg">
<img alt="github-dimmed theme" width="400" src="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/activity-github-dimmed.svg">
<img alt="dracula theme" width="400" src="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/activity-dracula.svg">
<img alt="nord theme" width="400" src="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/activity-nord.svg">
<img alt="tokyo-night theme" width="400" src="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/activity-tokyo-night.svg">
<img alt="solarized-light theme" width="400" src="https://raw.githubusercontent.com/spatelpatidar/repobeats/output/activity-solarized-light.svg">

## Use it

Add a step to a workflow. It has the same shape as `lowlighter/metrics`:

```yaml
- uses: actions/checkout@v4

- name: Repobeats - respondo-grpc
  uses: spatelpatidar/repobeats@v0.3.0
  continue-on-error: true
  timeout-minutes: 10
  with:
    token: ${{ secrets.METRICS_TOKEN }}
    user: spatelpatidar
    repo: respondo-grpc
    card: activity
    theme: dracula
    filename: repobeats.respondo-grpc.svg
    output_action: none
```

`output_action: none` leaves the file in the workspace for later steps. `output_action: commit` commits and pushes it to the checked-out branch (the job needs `permissions: contents: write`). A complete workflow with a schedule is in [`examples/repobeats.yml`](examples/repobeats.yml).

Then embed the file:

```md
![Repobeats](./repobeats.respondo-grpc.svg)
```

## Inputs

| Input | Default | Notes |
| --- | --- | --- |
| `token` | empty | See [Tokens](#tokens). |
| `user` | current repo owner | Only needed when `repo` is a bare name. |
| `repo` | current repository | `owner/name`, or a bare name together with `user`. |
| `card` | `activity` | `activity`, `languages`, `contributors` or `overview`. |
| `theme` | `light` | `light`, `dark`, `github-dimmed`, `dracula`, `nord`, `tokyo-night`, `solarized-light`. |
| `size` | `medium` | Width: `small` (610px), `medium` (814px), `large` (1221px). Height follows the card. |
| `period` | `30` | `7`, `14`, `30`, `90`, `180` or `365` days. Used by `activity` and `contributors`. |
| `filename` | `repobeats.svg` | Path relative to the workspace. Parent folders are created. |
| `max_pages` | `50` | API pages (100 items each) read per list. Raise it for very busy repositories. |
| `output_action` | `none` | `none` or `commit`. |
| `commit_message` | `chore: update repobeats card` | Used with `commit`. |

## Cards

| Card | Shows | API requests |
| --- | --- | --- |
| `activity` | Contributions header, issue ratio, pull requests opened, pushes, issue/PR/push charts, top 5 contributors, change against the previous period. | Commits and issues, 730 days, paged. |
| `contributors` | Up to 10 contributors for the period, with a bar and share per person. | Same as `activity`. |
| `languages` | Stacked bar and legend of the 8 largest languages, the rest grouped as "Other". | 1 |
| `overview` | Stars, forks, open issues, watchers, license, main language, last push, latest release, description. | 2 |

`languages` and `overview` are cheap and work well without a token on a single public repository. `activity` and `contributors` read two years of history, so on busy repositories pass a token and raise `max_pages` if the step reports a pagination error.

## Tokens

| Case | Token |
| --- | --- |
| Public repository | None needed, but anonymous requests are limited to 60 per hour per IP address. Pass `secrets.GITHUB_TOKEN` to avoid that. |
| The repository running the workflow, public or private | `${{ secrets.GITHUB_TOKEN }}` is enough. |
| A different private repository | A personal access token stored as a repository secret. Fine-grained: read-only access to **Contents**, **Issues**, **Pull requests** and **Metadata** on that one repository. Classic: `repo` scope. |

The token is passed to the Action through an environment variable and only used for `GET` requests.

## What ends up in the SVG

Counts, the GitHub login (or commit author name) of the top contributors, and for `overview` the repository description, license and latest release tag. If the repository is private and you commit the SVG to a public repository, those details become public. Commit it to a private repository, or keep it as a workflow artifact, if that matters.

## Releases are automatic

The `version` in `Cargo.toml` decides what is released when a pull request is merged into `main`. There is one branch and no tags to make by hand.

| `version` in `Cargo.toml` | Release | Git tag | Docker image tags | GitHub Release |
| --- | --- | --- | --- | --- |
| `0.4.0-beta.1` | pre-release | `v0.4.0-beta.1` | `0.4.0-beta.1`, `v0.4.0-beta.1`, `beta` | marked **Pre-release** |
| `0.4.0` | stable | `v0.4.0` | `0.4.0`, `v0.4.0`, `v0`, `latest` | marked **Latest** |

A pre-release never moves `latest` or `v0`, so people on `@v0` only get stable releases. Other suffixes work too: `0.4.0-rc.1` is a pre-release with the image tag `rc`.

**Workflow**
1. Work on a feature branch. When you want to publish a beta, change the version to `0.4.0-beta.1` in `Cargo.toml` and update the lock file with `cargo build`. Commit both files.
2. Merge the pull request. A pre-release and a `beta` image are published. Try it with `uses: <owner>/repobeats@v0.4.0-beta.1`.
3. For more betas, raise the number (`0.4.0-beta.2`) in the next pull request.
4. To ship, open a pull request that sets the version to `0.4.0` (and updates `Cargo.lock`). When it merges, the stable release and the `latest` image are published.

If a merged pull request does not change the version, the tag already exists and nothing is released. Documentation and showcase changes do not run the workflow at all. Add `[skip release]` to a commit message to skip it on a code change.

**What each run does:** it runs `cargo fmt --check`, `clippy` and `cargo test --locked` (a failure stops the release, and `--locked` also fails when `Cargo.lock` does not match the version), builds and pushes the Docker image to `ghcr.io/<owner>/repobeats`, creates the tag (the tagged commit carries an `action.yml` pinned to that image, and no extra commit lands on `main`), and creates the GitHub Release with generated notes.

The version must look like `0.4.0` or `0.4.0-beta.1`. `0.4.0.beta` is not valid in Cargo. If a run fails after the version check, fix the problem and use **Actions, Release, Run workflow** to retry the same version.

One-time setup:
- After the first release, open the package on GitHub (your profile, Packages, `repobeats`, Package settings) and set visibility to **Public**. Workflows can only pull the image anonymously if it is public.
- If your organization restricts workflow permissions, allow **Read and write permissions** under Settings, Actions, General.

In your own workflows: `@v0` follows the newest stable `0.x`, `@v0.4.1` pins a stable release, and `@v0.5.0-beta.1` tries a beta.

If you use another owner or name, change the `image:` line in `action.yml` and the `uses:` lines in this README and the examples. See [CONTRIBUTING.md](CONTRIBUTING.md) for the pull request steps and [BUILD_AND_TEST.md](BUILD_AND_TEST.md) if you are new to Rust.

## Run it locally

```bash
cargo build --release
GITHUB_TOKEN=ghp_xxx ./target/release/repobeats --repo owner/name --card languages --theme nord --filename card.svg
```

Every option is also available as a `REPOBEATS_<NAME>` environment variable (`REPOBEATS_REPO`, `REPOBEATS_CARD`, `REPOBEATS_PERIOD`, ...). `GITHUB_API_URL` is honored, so GitHub Enterprise Server works.

## How it works

- `src/collector.rs` reads commits and issues/pull requests for the last 730 days (two full 365-day windows, so any period can be compared with the one before it), plus languages, repository details and the latest release.
- `src/model.rs` buckets activity per day, ranks contributors and parses repository details.
- `src/card/render.rs` draws the activity card and holds the themes. `src/card/render/extra.rs` draws the languages, contributors and overview cards.
- `src/main.rs` is the command line the Action calls.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Changes from the hosted version

The original project was a web service with GitHub OAuth sign-in, a GitHub App with admin verification, a database and a cache. All of that is removed. The activity card renderer and the activity model are unchanged apart from new themes.

## License

MIT. This project is a clean-room implementation and is not affiliated with Axiom.
