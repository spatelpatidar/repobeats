# Contributing

How to raise a pull request, and what happens to the Docker image and the release when it is merged.

## The short version

1. Branch from `main`, make the change, run the checks.
2. Decide the release type and set `version` in `Cargo.toml` to match (see the table below).
3. Open a pull request. CI runs the tests, builds the Docker image, and shows a **Release preview**.
4. Merge. The **Release** workflow publishes a pre-release, a stable release, or nothing, based on the version.

## 1. Set up

New to Rust? Follow [BUILD_AND_TEST.md](BUILD_AND_TEST.md) to install the tools. Then:

```bash
git checkout main && git pull
git checkout -b my-change
```

## 2. Make the change and check it

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

Changed `Dockerfile` or `entrypoint.sh`? Build and run the image once yourself:

```bash
docker build -t repobeats:test .

mkdir -p out
docker run --rm \
  -e GITHUB_WORKSPACE=/work \
  -e INPUT_TOKEN=ghp_yourtoken \
  -e INPUT_REPO=owner/name \
  -e INPUT_CARD=languages \
  -e INPUT_FILENAME=card.svg \
  -v "$PWD/out:/work" \
  repobeats:test
```

The card appears in `out/card.svg`. (On Windows PowerShell use `${PWD}/out` and put the command on one line.)

## 3. Choose the release type

The `version` in `Cargo.toml` decides what a merge publishes. Pick one:

| You want | Set `version` to | Merging publishes | Docker image tags |
| --- | --- | --- | --- |
| No release (docs, tests, wait) | leave it unchanged | nothing | none |
| A pre-release to try | `0.4.0-beta.1` | GitHub **Pre-release** `v0.4.0-beta.1` | `0.4.0-beta.1`, `v0.4.0-beta.1`, `beta` |
| Another pre-release | `0.4.0-beta.2` | Pre-release `v0.4.0-beta.2` | same pattern, `beta` moves |
| Production | `0.4.0` | **Latest** release `v0.4.0` | `0.4.0`, `v0.4.0`, `v0`, `latest` |

After you change the version, update the lock file and commit both files:

```bash
# edit version in Cargo.toml, then:
cargo build
git add Cargo.toml Cargo.lock
```

Rules:
- Use `0.4.0-beta.1`, not `0.4.0.beta`. Cargo does not accept the second form.
- A pre-release never moves `latest` or `v0`, so people on `@v0` only get production releases.
- If you forget `Cargo.lock`, CI fails with "the lock file needs to be updated". Run `cargo build` and commit it.
- Do not edit the image tag in `action.yml` for a release. The workflow does it in the tagged commit.

## 4. Open the pull request

```bash
git push -u origin my-change
```

Open the pull request into `main`. The template asks for the release type and the version. CI then runs three checks:

| Check | What it does |
| --- | --- |
| `test` | `cargo fmt --check`, `clippy`, `cargo test --locked` |
| `docker` | Builds the Docker image, then runs `repobeats --help` inside it to prove it starts. Nothing is pushed. |
| `release-preview` | Reads `version` and writes what merging will publish. Open the run, then **Summary**. |

Example preview text: `Pre-release v0.4.0-beta.1. Merging publishes a GitHub pre-release and a Docker image tagged 0.4.0-beta.1 and beta. latest is not touched.` If it says **No release**, the tag already exists, so the version was not changed.

## 5. After you merge

The **Release** workflow starts on `main` (Actions tab). It runs in this order:

1. Tests again.
2. Reads the version. If the tag already exists, it stops here. Nothing is published.
3. Builds the Docker image and pushes it to `ghcr.io/<owner>/repobeats` with the tags from the table above.
4. Creates the git tag, with `action.yml` pinned to that image. `main` gets no extra commit.
5. Creates the GitHub Release (marked Pre-release or Latest) with generated notes.

Docs-only changes (README, showcase workflow) do not start the workflow. Add `[skip release]` to a commit message to skip it on a code change.

Use the result in a workflow:

```yaml
uses: spatelpatidar/repobeats@v0.4.0-beta.1   # try a pre-release
uses: spatelpatidar/repobeats@v0.4.0          # pin production
uses: spatelpatidar/repobeats@v0              # newest production 0.x
```

## 6. Typical timeline

| Step | Pull request changes `version` to | Result |
| --- | --- | --- |
| 1. New feature | `0.4.0-beta.1` | Pre-release `v0.4.0-beta.1`, image `beta` |
| 2. Fix found while testing | `0.4.0-beta.2` | Pre-release `v0.4.0-beta.2`, image `beta` |
| 3. A pull request with no version change | unchanged | Tests run, nothing published |
| 4. Ready for production | `0.4.0` | Release `v0.4.0` (Latest), image `latest` and `v0` |

## 7. When something fails

| Problem | Fix |
| --- | --- |
| `docker` check fails on the pull request | Run `docker build -t repobeats:test .` locally and read the error. |
| Release run failed after the version check | Fix the cause, then **Actions, Release, Run workflow** to retry the same version. No new version is needed if no tag was created. |
| Release says the tag already exists | The version was not changed. Change it in a new pull request. |
| `unauthorized` when a workflow pulls the image | The package is private. Open the package settings on GitHub and set visibility to **Public**. |
| Merged a lower version than the latest stable | The workflow has no downgrade check and would move `latest` back. Do not do this. Raise the version instead. |
