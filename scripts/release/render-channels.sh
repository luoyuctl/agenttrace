#!/usr/bin/env bash
set -euo pipefail

usage() {
	cat <<'USAGE'
Usage: scripts/release/render-channels.sh <version> <checksums-file> <output-dir>

Renders the Homebrew Formula for a published agenttrace GitHub Release. <version> may include a leading "v".
USAGE
}

fail() {
	echo "render-channels: $*" >&2
	exit 1
}

[[ $# -eq 3 ]] || {
	usage >&2
	exit 2
}

version="${1#v}"
checksums_file="$2"
output_dir="$3"
repo="luoyuctl/agenttrace"

[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "invalid version: $version"
[[ -f "$checksums_file" ]] || fail "checksums file does not exist: $checksums_file"

checksum_for() {
	local asset="$1"
	local checksum
	checksum="$(awk -v asset="$asset" '$2 { name = $2; sub(/^\*/, "", name); if (name == asset) { print $1; exit } }' "$checksums_file")"
	[[ "$checksum" =~ ^[[:xdigit:]]{64}$ ]] || fail "missing or invalid checksum for $asset"
	printf '%s' "$checksum"
}

linux_amd64="$(checksum_for agenttrace-linux-amd64)"
linux_arm64="$(checksum_for agenttrace-linux-arm64)"
darwin_amd64="$(checksum_for agenttrace-darwin-amd64)"
darwin_arm64="$(checksum_for agenttrace-darwin-arm64)"

homebrew_dir="$output_dir/homebrew/Formula"
mkdir -p "$homebrew_dir"

cat >"$homebrew_dir/agenttrace.rb" <<FORMULA
class Agenttrace < Formula
  desc "TUI observability for AI coding-agent session history, cost, latency, and anomalies"
  homepage "https://github.com/$repo"
  version "$version"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/$repo/releases/download/v$version/agenttrace-darwin-arm64"
      sha256 "$darwin_arm64"
    else
      url "https://github.com/$repo/releases/download/v$version/agenttrace-darwin-amd64"
      sha256 "$darwin_amd64"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/$repo/releases/download/v$version/agenttrace-linux-arm64"
      sha256 "$linux_arm64"
    else
      url "https://github.com/$repo/releases/download/v$version/agenttrace-linux-amd64"
      sha256 "$linux_amd64"
    end
  end

  def install
    bin.install Dir["agenttrace-*"].first => "agenttrace"
    chmod 0755, bin/"agenttrace"
  end

  test do
    assert_match "agenttrace v$version", shell_output("\#{bin}/agenttrace --version")
  end
end
FORMULA

echo "Rendered Homebrew Formula for v$version in $output_dir"
