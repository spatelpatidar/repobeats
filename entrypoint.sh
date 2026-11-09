#!/bin/sh
# GitHub passes action inputs to Docker actions as INPUT_<NAME> variables.
set -eu

export REPOBEATS_TOKEN="${INPUT_TOKEN:-}"
export REPOBEATS_USER="${INPUT_USER:-}"
export REPOBEATS_REPO="${INPUT_REPO:-}"
export REPOBEATS_CARD="${INPUT_CARD:-}"
export REPOBEATS_THEME="${INPUT_THEME:-}"
export REPOBEATS_SIZE="${INPUT_SIZE:-}"
export REPOBEATS_PERIOD="${INPUT_PERIOD:-}"
export REPOBEATS_COLORS="${INPUT_COLORS:-}"
export REPOBEATS_ANIMATE="${INPUT_ANIMATE:-}"
export REPOBEATS_FILENAME="${INPUT_FILENAME:-}"
export REPOBEATS_MAX_PAGES="${INPUT_MAX_PAGES:-}"

cd "${GITHUB_WORKSPACE:-.}"
repobeats

if [ "${INPUT_OUTPUT_ACTION:-none}" = "commit" ]; then
  file="${INPUT_FILENAME:-repobeats.svg}"
  git config --global --add safe.directory "$PWD"
  if git status --porcelain -- "$file" | grep -q .; then
    git config user.name "github-actions[bot]"
    git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
    git add -- "$file"
    git commit -m "${INPUT_COMMIT_MESSAGE:-chore: update repobeats card}"
    git push origin HEAD
  else
    echo "Card unchanged, nothing to commit."
  fi
fi
