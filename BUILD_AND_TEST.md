# Build and test Repobeats (no Rust knowledge needed)

You only need to copy the commands below. Rust is the language the project is written in. Cargo is its build tool, like npm for Node.

## 1. Install Rust (once)

**Windows**
1. Download and run `rustup-init.exe` from https://rustup.rs
2. If it asks for the Visual Studio C++ Build Tools, accept and install them. Rust needs them on Windows.
3. Close and reopen your terminal.

**macOS / Linux**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Then close and reopen your terminal.

Check it worked:
```bash
cargo --version
```
You should see a version number. The project needs Rust 1.85 or newer (edition 2024). If yours is older, run `rustup update`.

## 2. Build

From the project folder (the one that has `Cargo.toml`):

```bash
cargo build --release
```

The first build takes a few minutes because it downloads and compiles dependencies. Later builds take seconds.

The program is created at:
- Linux / macOS: `target/release/repobeats`
- Windows: `target\release\repobeats.exe`

## 3. Run the tests

```bash
cargo test
```

Look for `test result: ok` at the end. Currently 26 tests pass. If something fails, the output shows which test and why.

These tests check the card drawing, the day counting and the repository name checks. They do not call GitHub, so they need no token and no internet after the first build.

## 4. Check code style (CI runs these too)

```bash
cargo fmt            # fixes formatting automatically
cargo fmt --check    # only checks, changes nothing
cargo clippy --all-targets -- -D warnings
```

If CI fails on formatting, run `cargo fmt`, commit the result and push.

## 5. Make a card on your computer

You need a GitHub token. Create a fine-grained personal access token with read-only access to Contents, Issues and Pull requests on the repository. For a public repository you can skip the token for a quick try, but GitHub may block you after a few requests.

**Linux / macOS**
```bash
export GITHUB_TOKEN=ghp_yourtoken
./target/release/repobeats --repo spatelpatidar/respondo-grpc --theme dark --period 90 --filename card.svg
```

**Windows PowerShell**
```powershell
$env:GITHUB_TOKEN = "ghp_yourtoken"
.\target\release\repobeats.exe --repo spatelpatidar/respondo-grpc --theme dark --period 90 --filename card.svg
```

Open `card.svg` in a browser to see it.

Options: `--repo owner/name`, `--card activity|languages|contributors|overview`, `--theme light|dark|github-dimmed|dracula|nord|tokyo-night|solarized-light`, `--size small|medium|large`, `--period 7|14|30|90|180|365`, `--filename file.svg`, `--max-pages N`. Run `repobeats --help` to list them.

Try the other cards the same way, for example `--card languages --theme dracula` or `--card overview`.

Without installing anything permanently you can also run it directly through Cargo:
```bash
cargo run --release -- --repo owner/name --filename card.svg
```
(The `--` separates Cargo's options from the program's options.)

## 6. Build the Docker image (optional)

You only need this to test the image on your own machine. GitHub builds it for you when you push a tag.

```bash
docker build -t repobeats:test .
```

Run it the way GitHub Actions does:

```bash
mkdir -p out
docker run --rm \
  -e GITHUB_WORKSPACE=/work \
  -e INPUT_TOKEN=ghp_yourtoken \
  -e INPUT_REPO=spatelpatidar/respondo-grpc \
  -e INPUT_FILENAME=card.svg \
  -v "$PWD/out:/work" \
  repobeats:test
```

The card appears in `out/card.svg`. On Windows PowerShell, replace `$PWD/out` with `${PWD}/out` and put the command on one line, or end each line with a backtick instead of `\`.

## 7. Release a new version

Releases are automatic and controlled by one number: `version` in `Cargo.toml`. You do not make tags.

**Beta (pre-release)**
1. Create a branch and make your change.
2. In `Cargo.toml`, set `version = "0.4.0-beta.1"`.
3. Run `cargo build`. This updates `Cargo.lock`, which must change together with `Cargo.toml`.
4. Commit `Cargo.toml` and `Cargo.lock` with your change, push, open a pull request and merge it.
5. In the **Actions** tab the **Release** run publishes `v0.4.0-beta.1` as a Pre-release and pushes a Docker image tagged `beta`. It never touches `latest`.
6. Test it with `uses: spatelpatidar/repobeats@v0.4.0-beta.1`. For the next beta use `0.4.0-beta.2`, and so on.

**Stable (production)**
1. Open a pull request that sets `version = "0.4.0"` and run `cargo build` to update `Cargo.lock`.
2. Merge it. The Release run publishes `v0.4.0` as the Latest release and pushes the image tagged `0.4.0`, `v0.4.0`, `v0` and `latest`.

**Rules**
- A merged pull request that does not change the version releases nothing, because the tag already exists.
- A change that only touches docs, the README or the showcase workflow does not run the release at all.
- Add `[skip release]` to the merge commit message to skip a release on a code change.
- The version must look like `0.4.0` or `0.4.0-beta.1`. `0.4.0.beta` is not valid.
- If a release run fails, fix the cause and use **Actions, Release, Run workflow** to retry the same version.
- You never edit the image tag in `action.yml` for a release. The workflow writes it into the tagged commit.

After the first release, set the `repobeats` package to Public (profile, Packages, Package settings).

## 8. Where things are

| File | What it does |
| --- | --- |
| `src/main.rs` | Reads the options and writes the SVG file. |
| `src/collector.rs` | Gets commits, issues and pull requests from GitHub. |
| `src/model.rs` | Counts activity per day and ranks contributors. |
| `src/card/render.rs` | Draws the activity card and defines the themes (colors). |
| `src/card/render/extra.rs` | Draws the languages, contributors and overview cards. |
| `src/card/options.rs` | Allowed card types, themes, sizes and periods. |
| `action.yml` | The GitHub Action definition and its inputs. |
| `entrypoint.sh` | Connects Action inputs to the program and does the optional commit. |
| `Dockerfile` | Builds the image. |
| `.github/workflows/ci.yml` | Runs format, lint and tests on every push. |
| `.github/workflows/release.yml` | On merge to `main`: reads the version from `Cargo.toml`, tests, builds and pushes the image, tags, creates a pre-release or a stable release. |
| `.github/workflows/showcase.yml` | Makes the example cards and publishes them to the `output` branch. |

To change theme colors, edit `Palette::for_theme` in `src/card/render.rs`. To add a theme, add a name in `src/card/options.rs` and a color set in `Palette::for_theme`. To change a card's layout or text, edit `src/card/render.rs` (activity) or `src/card/render/extra.rs` (the others). After a change, run `cargo test`. Some tests check exact text such as "Contributions in the Last 30 Days", so update the matching test in `tests/card.rs` if you change a label on purpose.

## 9. Common problems

| Message | Meaning and fix |
| --- | --- |
| `cargo: command not found` | Rust is not installed, or the terminal was not reopened after installing. |
| `linker 'link.exe' not found` (Windows) | Install the Visual Studio C++ Build Tools, then try again. |
| `the lock file needs to be updated but --locked was passed` | `Cargo.toml` changed but `Cargo.lock` did not. Run `cargo build`, then commit both files. |
| `Diff in ...` from `cargo fmt --check` | Formatting issue. Run `cargo fmt` and commit. |
| `rate limit` or `403` from GitHub | No token, or too many requests. Set `GITHUB_TOKEN`. |
| `404` from GitHub | Wrong repository name, or the token cannot see that private repository. |
| `unauthorized` when the workflow pulls the image | The package on GHCR is private. Set it to Public in the package settings. |
| Release run skipped, or "tag ... already exists" | The version in `Cargo.toml` did not change, no code files changed, or the message had `[skip release]`. |
| `the lock file needs to be updated but --locked was passed` | You changed `version` in `Cargo.toml` without running `cargo build`. Run it and commit `Cargo.lock` too. |
| Tag push denied | Check Settings, Actions, General, Workflow permissions, and any tag protection rule for `v*`. |
