# Repobeats

A GitHub repository activity card, generated as an SVG file by a GitHub Action in your own workflow.

There is no sign-in, no GitHub App to install, no hosted service and no database. The Action runs on your runner as a prebuilt Docker image, reads the repository with a token you provide, and writes an SVG file. Your token and your repository data never go to a third party; the only network calls are to `api.github.com`.

The card shows pushes, opened and closed issues, opened and closed pull requests, the five most active contributors, and the change against the previous period of the same length.

## Use it

Add a step to a workflow. It has the same shape as `lowlighter/metrics`:

```yaml
- uses: actions/checkout@v4

- name: Repobeats - respondo-grpc
  uses: spatelpatidar/repobeats@v0.2.0
  continue-on-error: true
  timeout-minutes: 10
  with:
    token: ${{ secrets.METRICS_TOKEN }}
    user: spatelpatidar
    repo: respondo-grpc
    filename: repobeats.respondo-grpc.svg
    output_action: none
```

`output_action: none` leaves the file in the workspace for later steps. `output_action: commit` commits and pushes it to the checked-out branch (the job needs `permissions: contents: write`). A complete workflow with a schedule is in [`examples/repobeats.yml`](examples/repobeats.yml).

Then embed the committed file:

```md
![Repobeats](./repobeats.respondo-grpc.svg)
```

## Inputs

| Input | Default | Notes |
| --- | --- | --- |
| `token` | empty | See [Tokens](#tokens). |
| `user` | current repo owner | Only needed when `repo` is a bare name. |
| `repo` | current repository | `owner/name`, or a bare name together with `user`. |
| `theme` | `light` | `light` or `dark`. |
| `size` | `medium` | `small` (610×239), `medium` (814×318), `large` (1221×477). |
| `period` | `30` | `7`, `14`, `30`, `90`, `180` or `365` days. |
| `filename` | `repobeats.svg` | Path relative to the workspace. Parent folders are created. |
| `max_pages` | `50` | API pages (100 items each) read per list. Raise it for very busy repositories. |
| `output_action` | `none` | `none` or `commit`. |
| `commit_message` | `chore: update repobeats card` | Used with `commit`. |

## Tokens

| Case | Token |
| --- | --- |
| Public repository | None needed, but anonymous requests are limited to 60 per hour. Pass `secrets.GITHUB_TOKEN` to avoid that. |
| The repository running the workflow, public or private | `${{ secrets.GITHUB_TOKEN }}` is enough. |
| A different private repository | A personal access token stored as a repository secret. Fine-grained: read-only access to **Contents**, **Issues** and **Pull requests** on that one repository. Classic: `repo` scope. |

The token is passed to the Action through an environment variable and only used for `GET` requests.

## What ends up in the SVG

Counts of pushes, issues and pull requests, plus the GitHub login (or commit author name) of the top five contributors. If the repository is private and you commit the SVG to a public repository, those names and numbers become public. Commit it to a private repository, or keep it as a workflow artifact, if that matters.

## Publish your own release

1. Create the repository (the files here assume `spatelpatidar/repobeats`; if you use another owner or name, change the `image:` line in `action.yml` and the `uses:` lines in the README and example).
2. Push the code, then tag a release: `git tag v0.2.0 && git push origin v0.2.0`.
3. `.github/workflows/release.yml` builds the Docker image and pushes it to `ghcr.io/<owner>/repobeats` as `0.2.0`, `v0.2.0`, `v0` and `latest`.
4. Open the package on GitHub (Packages, `repobeats`, Package settings) and set visibility to **Public**. Workflows can only pull the image anonymously if it is public. Your card steps then run with `uses: <owner>/repobeats@v0.2.0`.

For a new version, bump `version` in `Cargo.toml`, update the tag in `action.yml`'s `image:` line, commit, and tag again.

## Run it locally

```bash
cargo build --release
GITHUB_TOKEN=ghp_xxx ./target/release/repobeats --repo owner/name --theme dark --period 90 --filename card.svg
```

Every option is also available as a `REPOBEATS_<NAME>` environment variable (`REPOBEATS_REPO`, `REPOBEATS_PERIOD`, ...). `GITHUB_API_URL` is honored, so GitHub Enterprise Server works.

## How it works

- `src/collector.rs` reads commits and issues/pull requests for the last 730 days with the token (two full 365-day windows, so any period can be compared with the one before it).
- `src/model.rs` buckets them per day and ranks contributors.
- `src/card/` renders the SVG.
- `src/main.rs` is the command line the Action calls.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Changes from the hosted version

The original project was a web service with GitHub OAuth sign-in, a GitHub App with admin verification, a database and a cache. All of that is removed. The card renderer and activity model are unchanged.

## License

MIT. This project is a clean-room implementation and is not affiliated with Axiom.
