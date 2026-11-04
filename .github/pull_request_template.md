## What changed

<!-- One or two sentences. Link the issue if there is one. -->

## Release type

<!-- Tick one. The version in Cargo.toml decides what merging publishes. -->

- [ ] **No release.** Docs or tests only, or a change that should wait. `version` is unchanged.
- [ ] **Pre-release.** `version` is now `X.Y.Z-beta.N` (Docker image tagged `beta`, `latest` untouched).
- [ ] **Stable release.** `version` is now `X.Y.Z` (Docker image tagged `latest`).

Version in `Cargo.toml`: `...`

## Checklist

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass locally
- [ ] If I changed `version`, I ran `cargo build` and committed `Cargo.lock` too
- [ ] If I changed the Dockerfile or `entrypoint.sh`, I built the image locally (`docker build -t repobeats:test .`)
- [ ] I checked the **Release preview** on this pull request and it says what I expect
