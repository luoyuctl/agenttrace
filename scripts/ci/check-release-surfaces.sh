#!/usr/bin/env bash
set -euo pipefail

fail() {
  echo "check-release-surfaces: $*" >&2
  exit 1
}

version="$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -1)"
[[ -n "$version" ]] || fail "could not read workspace package version"

grep -q "Homebrew-tap-" README.md \
  || fail "README must link to the Homebrew tap without claiming an unpublished version"
grep -q "Homebrew-tap-" README.zh-CN.md \
  || fail "README.zh-CN must link to the Homebrew tap without claiming an unpublished version"
grep -q "AGENTTRACE v$version" README.md \
  || fail "README sample output is not aligned with engine version $version"
grep -q "version \"$version\"" homebrew/Formula/agenttrace.rb \
  || fail "Homebrew formula version is not aligned with engine version $version"
grep -q "agenttrace v$version" homebrew/Formula/agenttrace.rb \
  || fail "Homebrew formula test is not aligned with engine version $version"
grep -q "\"softwareVersion\": \"$version\"" site/index.html \
  || fail "site structured metadata is not aligned with engine version $version"
grep -q "\"version\": \"$version\"" .codex-plugin/plugin.json \
  || fail "Codex plugin manifest is not aligned with source-tree version $version"
grep -q "cargo build --release -p agenttrace" install.sh \
  || fail "install.sh source-build fallback must use cargo"
grep -q "cargo build --release -p agenttrace" install.ps1 \
  || fail "install.ps1 source-build fallback must use cargo"
grep -q "cargo install" README.md \
  || fail "README install paths must include cargo install"
grep -q "cargo install" README.zh-CN.md \
  || fail "README.zh-CN install paths must include cargo install"
grep -q "depends_on \"rust\" => :build" homebrew/Formula/agenttrace.rb \
  || fail "Homebrew formula must use Rust build dependency"
grep -Eq '"cargo".*"install".*"crates/agenttrace-cli"' homebrew/Formula/agenttrace.rb \
  || fail "Homebrew formula must install the Rust CLI crate"

for target in \
  x86_64-unknown-linux-gnu \
  aarch64-unknown-linux-gnu \
  x86_64-apple-darwin \
  aarch64-apple-darwin \
  x86_64-pc-windows-msvc \
  aarch64-pc-windows-msvc
do
  grep -q "target: $target" .github/workflows/release.yml \
    || fail "release workflow missing Rust target $target"
done

for asset in \
  agenttrace-linux-amd64 \
  agenttrace-linux-arm64 \
  agenttrace-darwin-amd64 \
  agenttrace-darwin-arm64 \
  agenttrace-windows-amd64.exe \
  agenttrace-windows-arm64.exe
do
  grep -q "asset: $asset" .github/workflows/release.yml \
    || fail "release workflow missing asset $asset"
done

grep -q "cargo build --release -p agenttrace --target" .github/workflows/release.yml \
  || fail "release workflow must build Rust release binaries"
grep -q "dtolnay/rust-toolchain" .github/workflows/release.yml \
  || fail "release workflow must set up Rust"
grep -q "gh release create" .github/workflows/release.yml \
  || fail "release workflow must publish GitHub releases"

while IFS= read -r file; do
  [[ ! -e "$file" ]] || fail "npm wrapper files should not be tracked"
done < <(git ls-files 'npm/*')

if grep -R -Eqi "npm (wrapper|package)|npm install -g agenttrace|AGENTTRACE_RELEASE_TAG|npm/" \
  README.md README.zh-CN.md CONTRIBUTING.md CHANGELOG.md docs/launch-kit.md site homebrew; then
  fail "public release surfaces must not advertise npm package support"
fi

if grep -R -Eqi "go install github.com/luoyuctl/agenttrace|go build .*cmd/agenttrace|setup-go|go-version-file" \
  README.md README.zh-CN.md CONTRIBUTING.md docs/launch-kit.md docs/demo-playbook.md docs/parser-guide.md docs/agentops-prompt-rules.md install.sh install.ps1 .github homebrew site skills; then
  fail "public release surfaces must not advertise Go build/install paths"
fi

if grep -R -Eqi "pkg.go.dev|goreportcard|img.shields.io/badge/go-" README.md README.zh-CN.md site docs/launch-kit.md; then
  fail "README badges must advertise the Rust default implementation, not Go"
fi

if grep -R -Eqi "built in Go|Bubble Tea terminal UI" README.md README.zh-CN.md site .codex-plugin skills; then
  fail "public surfaces must describe the Rust ratatui implementation"
fi

for asset in assets/readme-real-overview.png assets/readme-real-diagnostics.png; do
  [[ -f "$asset" ]] || fail "plugin screenshot asset is missing: $asset"
  grep -q "\"./$asset\"" .codex-plugin/plugin.json \
    || fail "plugin manifest must reference existing screenshot $asset"
done

if grep -q "crates.io/crates/agenttrace" site/index.html; then
  fail "site must not advertise crates.io before the crate exists"
fi

if grep -R -Eqi "setup-go|(^|[^[:alnum:]_/-])go[[:space:]]+build|GOOS|GOARCH" .github/workflows; then
  fail "GitHub workflows must not use Go release/build matrix settings"
fi

if [[ -e go.mod || -e go.sum || -d cmd || -d internal ]]; then
  fail "Go implementation files must not be present in the Rust-only tree"
fi

if git ls-files '*.go' go.mod go.sum 'cmd/**' 'internal/**' | while IFS= read -r path; do [[ -e "$path" ]] && printf '%s\n' "$path"; done | grep -q .; then
  fail "Go implementation files must not be tracked in the Rust-only tree"
fi

if ! grep -q "<div class=\"meta\">v$version" site/demo-report.html &&
  ! grep -qi "static sample data" site/demo-report.html; then
  fail "site demo report must use current version metadata or clearly identify static sample data"
fi

node -e '
const fs = require("fs");
const version = process.argv[1];
const files = ["README.md", "README.zh-CN.md", "homebrew/Formula/agenttrace.rb", "site/index.html", "site/demo-report.html"];
const pattern = /\bv?(\d+\.\d+\.\d+)\b/g;
for (const file of files) {
  const text = fs.readFileSync(file, "utf8");
  for (const match of text.matchAll(pattern)) {
    if (match[1] !== version) {
      throw new Error(`${file} contains stale version ${match[0]}, expected ${version}`);
    }
  }
}
' "$version" || fail "release surface version drift detected"
