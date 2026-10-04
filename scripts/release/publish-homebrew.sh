#!/usr/bin/env bash
# Push a rendered Homebrew Formula to luoyuctl/homebrew-tap.
# Usage: publish-homebrew.sh <tag> <rendered-formula>
set -euo pipefail

tag="${1:?usage: publish-homebrew.sh <tag> <rendered-formula>}"
formula="${2:?usage: publish-homebrew.sh <tag> <rendered-formula>}"

[[ -n "${HOMEBREW_TAP_TOKEN:-}" ]] || {
	echo "HOMEBREW_TAP_TOKEN is required to update luoyuctl/homebrew-tap." >&2
	exit 1
}
[[ -f "$formula" ]] || {
	echo "rendered formula not found: $formula" >&2
	exit 1
}

tap_dir="$(mktemp -d "${TMPDIR:-/tmp}/homebrew-tap.XXXXXX")"
trap 'rm -rf "$tap_dir"' EXIT

git clone --depth 1 "https://x-access-token:${HOMEBREW_TAP_TOKEN}@github.com/luoyuctl/homebrew-tap.git" "$tap_dir"
install -Dm644 "$formula" "$tap_dir/Formula/agenttrace.rb"
cd "$tap_dir"
git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add Formula/agenttrace.rb
if git diff --cached --quiet; then
	echo "Homebrew Formula already up to date for ${tag}"
	exit 0
fi
git commit -m "agenttrace ${tag}"
git push origin HEAD
