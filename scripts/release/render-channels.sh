#!/usr/bin/env bash
set -euo pipefail

usage() {
	cat <<'USAGE'
Usage: scripts/release/render-channels.sh <version> <checksums-file> <output-dir>

Renders the Homebrew Formula and Microsoft WinGet manifests for a published
agenttrace GitHub Release. <version> may include a leading "v".
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
windows_amd64="$(checksum_for agenttrace-windows-amd64.exe)"
windows_arm64="$(checksum_for agenttrace-windows-arm64.exe)"

homebrew_dir="$output_dir/homebrew/Formula"
winget_dir="$output_dir/winget/manifests/l/Luoyuctl/AgentTrace/$version"
mkdir -p "$homebrew_dir" "$winget_dir"

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

cat >"$winget_dir/Luoyuctl.AgentTrace.yaml" <<VERSION
PackageIdentifier: Luoyuctl.AgentTrace
PackageVersion: $version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.10.0
VERSION

cat >"$winget_dir/Luoyuctl.AgentTrace.locale.en-US.yaml" <<LOCALE
PackageIdentifier: Luoyuctl.AgentTrace
PackageVersion: $version
PackageLocale: en-US
Publisher: Luoyuctl
PublisherUrl: https://github.com/luoyuctl
PublisherSupportUrl: https://github.com/$repo/issues
PackageName: AgentTrace
PackageUrl: https://github.com/$repo
License: MIT
LicenseUrl: https://github.com/$repo/blob/master/LICENSE
ShortDescription: Local-first TUI and reports for AI coding-agent session history, cost, tokens, time, and slow-run diagnosis.
Description: AgentTrace is a local-first terminal TUI and report generator for AI coding-agent session history, cost, tokens, time, and slow-run diagnosis.
Moniker: agenttrace
Tags:
  - agent
  - ai
  - cli
  - observability
  - tui
ManifestType: defaultLocale
ManifestVersion: 1.10.0
LOCALE

cat >"$winget_dir/Luoyuctl.AgentTrace.installer.yaml" <<INSTALLER
PackageIdentifier: Luoyuctl.AgentTrace
PackageVersion: $version
InstallerType: portable
Commands:
  - agenttrace
Installers:
  - Architecture: x64
    InstallerUrl: https://github.com/$repo/releases/download/v$version/agenttrace-windows-amd64.exe
    InstallerSha256: $windows_amd64
    PortableCommandAlias: agenttrace
  - Architecture: arm64
    InstallerUrl: https://github.com/$repo/releases/download/v$version/agenttrace-windows-arm64.exe
    InstallerSha256: $windows_arm64
    PortableCommandAlias: agenttrace
ManifestType: installer
ManifestVersion: 1.10.0
INSTALLER

echo "Rendered Homebrew Formula and WinGet manifests for v$version in $output_dir"
