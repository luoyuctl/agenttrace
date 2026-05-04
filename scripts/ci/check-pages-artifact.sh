#!/usr/bin/env bash
set -euo pipefail

page_dir="${1:-site}"
repo_root="$(git rev-parse --show-toplevel)"

fail() {
  echo "check-pages-artifact: $*" >&2
  exit 1
}

[[ -d "$page_dir" ]] || fail "page directory not found: $page_dir"
[[ -f "$page_dir/index.html" ]] || fail "missing index.html in $page_dir"
[[ -f "$page_dir/demo-report.html" ]] || fail "missing demo-report.html in $page_dir"

version="$(sed -nE 's/^const Version = "([^"]+)"/\1/p' "$repo_root/internal/engine/engine.go")"
[[ -n "$version" ]] || fail "could not read internal/engine Version"

if ! grep -q "<div class=\"meta\">v$version" "$page_dir/demo-report.html" &&
  ! grep -qi "static sample data" "$page_dir/demo-report.html"; then
  fail "demo-report.html must use current version metadata or clearly identify static sample data"
fi

for asset in \
  assets/agenttrace-demo.gif \
  assets/hero-banner.png \
  assets/logo-icon.png \
  assets/readme-real-overview.png; do
  if [[ ! -f "$page_dir/$asset" && ! -f "$repo_root/$asset" ]]; then
    fail "missing Pages asset: $asset"
  fi
done

node - "$page_dir" "$repo_root" <<'NODE'
const fs = require("fs");
const path = require("path");

const pageDir = process.argv[2];
const repoRoot = process.argv[3];
const files = ["index.html", "demo-report.html"];
const attrPattern = /\b(?:href|src)=["']([^"']+)["']/g;

function exists(ref) {
  const clean = ref.split("#")[0].split("?")[0];
  if (!clean || clean.startsWith("http://") || clean.startsWith("https://") || clean.startsWith("data:") || clean.startsWith("mailto:")) {
    return true;
  }
  if (clean.startsWith("#")) return true;
  const pageRelative = path.join(pageDir, clean);
  const repoRelative = path.join(repoRoot, clean);
  return fs.existsSync(pageRelative) || fs.existsSync(repoRelative);
}

for (const file of files) {
  const html = fs.readFileSync(path.join(pageDir, file), "utf8");
  for (const match of html.matchAll(attrPattern)) {
    if (!exists(match[1])) {
      throw new Error(`${file} references missing local asset ${match[1]}`);
    }
  }
}
NODE
