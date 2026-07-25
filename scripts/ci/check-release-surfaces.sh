#!/usr/bin/env bash
set -euo pipefail

fail() {
	echo "check-release-surfaces: $*" >&2
	exit 1
}

workspace_version="$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -1)"
[[ "$workspace_version" = "0.0.0-dev" ]] ||
	fail "workspace must use the development-version placeholder"

grep -q "Homebrew-tap-" README.md ||
	fail "README must link to the Homebrew tap without claiming an unpublished version"
grep -q "Homebrew-tap-" README.zh-CN.md ||
	fail "README.zh-CN must link to the Homebrew tap without claiming an unpublished version"
grep -q "AGENTTRACE_RELEASE_VERSION" .github/workflows/release.yml ||
	fail "release workflow must derive the binary version from the release tag"
grep -q "manifest.version = process.env.RELEASE_VERSION" .github/workflows/release.yml ||
	fail "release workflow must derive the npm package version from the release tag"
grep -q "cargo build --release -p agenttrace" install.sh ||
	fail "install.sh source-build fallback must use cargo"
grep -q "cargo build --release -p agenttrace" install.ps1 ||
	fail "install.ps1 source-build fallback must use cargo"
grep -q "cargo install" README.md ||
	fail "README install paths must include cargo install"
grep -q "cargo install" README.zh-CN.md ||
	fail "README.zh-CN install paths must include cargo install"
grep -q "brew install luoyuctl/tap/agenttrace" README.md ||
	fail "README must document Homebrew installation"
grep -q "winget install --id Luoyuctl.AgentTrace --exact" README.md ||
	fail "README must document WinGet installation"
grep -q "npm install -g @zack78/agenttrace" README.md ||
	fail "README must document npm installation"
grep -q "npm install -g @zack78/agenttrace" README.zh-CN.md ||
	fail "README.zh-CN must document npm installation"
grep -q '"name": "@zack78/agenttrace"' npm/package.json ||
	fail "npm package must use the @zack78/agenttrace name"
grep -q '"postinstall": "node scripts/install.js"' npm/package.json ||
	fail "npm package must install the matching native release binary"
grep -q '"access": "public"' npm/package.json ||
	fail "npm package must declare public publish access"
grep -q "checksum mismatch" npm/scripts/install.js ||
	fail "npm installer must verify release checksums"
grep -q "render-channels.sh" .github/workflows/release.yml ||
	fail "release workflow must render package-channel artifacts"
grep -q "Publish npm launcher" .github/workflows/release.yml ||
	fail "release workflow must publish the npm launcher"
grep -q "Publish Homebrew Formula" .github/workflows/release.yml ||
	fail "release workflow must publish the Homebrew Formula"
grep -q "Luoyuctl.AgentTrace" scripts/release/render-channels.sh ||
	fail "release helper must render the WinGet package identifier"
grep -q "Submit WinGet manifest" .github/workflows/release.yml ||
	fail "release workflow must submit the WinGet manifest"
grep -q "WINGET_GITHUB_TOKEN" .github/workflows/release.yml ||
	fail "release workflow must use the WinGet submission token"

for target in \
	x86_64-unknown-linux-gnu \
	aarch64-unknown-linux-gnu \
	x86_64-apple-darwin \
	aarch64-apple-darwin \
	x86_64-pc-windows-msvc \
	aarch64-pc-windows-msvc; do
	grep -q "target: $target" .github/workflows/release.yml ||
		fail "release workflow missing Rust target $target"
done

for asset in \
	agenttrace-linux-amd64 \
	agenttrace-linux-arm64 \
	agenttrace-darwin-amd64 \
	agenttrace-darwin-arm64 \
	agenttrace-windows-amd64.exe \
	agenttrace-windows-arm64.exe; do
	grep -q "asset: $asset" .github/workflows/release.yml ||
		fail "release workflow missing asset $asset"
done

grep -q "cargo build --release -p agenttrace --target" .github/workflows/release.yml ||
	fail "release workflow must build Rust release binaries"
grep -q "dtolnay/rust-toolchain" .github/workflows/release.yml ||
	fail "release workflow must set up Rust"
grep -q "gh release create" .github/workflows/release.yml ||
	fail "release workflow must publish GitHub releases"

if grep -R -Eqi "go install github.com/luoyuctl/agenttrace|go build .*cmd/agenttrace|setup-go|go-version-file" \
	README.md README.zh-CN.md CONTRIBUTING.md docs/maintainers/launch-kit.md docs/maintainers/demo-playbook.md docs/guides/parser-guide.md docs/maintainers/agentops-prompt-rules.md install.sh install.ps1 .github homebrew skills; then
	fail "public release surfaces must not advertise Go build/install paths"
fi

if grep -R -Eqi "pkg.go.dev|goreportcard|img.shields.io/badge/go-" README.md README.zh-CN.md docs/maintainers/launch-kit.md; then
	fail "README badges must advertise the Rust default implementation, not Go"
fi

if grep -R -Eqi "built in Go|Bubble Tea terminal UI" README.md README.zh-CN.md .codex-plugin skills; then
	fail "public surfaces must describe the Rust ratatui implementation"
fi

for asset in assets/readme-real-overview.png assets/readme-real-diagnostics.png; do
	[[ -f "$asset" ]] || fail "plugin screenshot asset is missing: $asset"
	grep -q "\"./$asset\"" .codex-plugin/plugin.json ||
		fail "plugin manifest must reference existing screenshot $asset"
done

if grep -R -Eqi "setup-go|(^|[^[:alnum:]_/-])go[[:space:]]+build|GOOS|GOARCH" .github/workflows; then
	fail "GitHub workflows must not use Go release/build matrix settings"
fi

if [[ -e go.mod || -e go.sum || -d cmd || -d internal ]]; then
	fail "Go implementation files must not be present in the Rust-only tree"
fi

if git ls-files '*.go' go.mod go.sum 'cmd/**' 'internal/**' | while IFS= read -r path; do [[ -e "$path" ]] && printf '%s\n' "$path"; done | grep -q .; then
	fail "Go implementation files must not be tracked in the Rust-only tree"
fi

npm_version="$(node -p "require('./npm/package.json').version")"
[[ "$npm_version" = "0.0.0-release" ]] ||
	fail "npm package must keep the release-version placeholder outside release CI"
