#!/usr/bin/env bash
set -euo pipefail

fail() {
  echo "check-release-surfaces: $*" >&2
  exit 1
}

version="$(sed -nE 's/^const Version = "([^"]+)"/\1/p' internal/engine/engine.go)"
[[ -n "$version" ]] || fail "could not read internal/engine Version"

grep -q "Homebrew-v$version-" README.md \
  || fail "README Homebrew badge is not aligned with engine version $version"
grep -q "AGENTTRACE v$version" README.md \
  || fail "README sample output is not aligned with engine version $version"
grep -q "version \"$version\"" homebrew/Formula/agenttrace.rb \
  || fail "Homebrew formula version is not aligned with engine version $version"
grep -q "agenttrace v$version" homebrew/Formula/agenttrace.rb \
  || fail "Homebrew formula test is not aligned with engine version $version"
grep -q "\"softwareVersion\": \"$version\"" site/index.html \
  || fail "site structured metadata is not aligned with engine version $version"
grep -q "AGENTTRACE_RELEASE_TAG=v$version node install.js" npm/README.md \
  || fail "npm README maintainer release-tag check is not aligned with engine version $version"

npm_package_version="$(node -p 'require("./npm/package.json").version || ""')"
[[ "$npm_package_version" == "$version" ]] \
  || fail "npm package version $npm_package_version is not aligned with engine version $version"

if grep -Eqi "not been published yet|registry 404|until the first publish" npm/README.md &&
  grep -q "^npm install -g agenttrace$" npm/README.md &&
  ! grep -qi "After the package is published" npm/README.md; then
  fail "npm README must not present npm install as active while the package is unpublished"
fi

if grep -Eqi "not been published yet|registry 404|until the first publish" npm/README.md &&
  grep -Eqi "npm wrapper is also available|npm install -g agenttrace" README.md &&
  ! grep -Eqi "npm wrapper package is not published yet|after the package is published" README.md; then
  fail "README must not present npm as active while the package is unpublished"
fi

if ! grep -q "<div class=\"meta\">v$version" site/demo-report.html &&
  ! grep -qi "static sample data" site/demo-report.html; then
  fail "site demo report must use current version metadata or clearly identify static sample data"
fi

node -e '
const fs = require("fs");
const version = process.argv[1];
const files = ["README.md", "homebrew/Formula/agenttrace.rb", "site/index.html", "site/demo-report.html", "npm/README.md", "npm/package.json"];
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
