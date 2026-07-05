#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

fail() {
  echo "check-cargo-manifests: $*" >&2
  exit 1
}

metadata_file="$(mktemp "${TMPDIR:-/tmp}/agenttrace-cargo-metadata.XXXXXX")"
cleanup() {
  rm -f "$metadata_file"
}
trap cleanup EXIT

cargo metadata --no-deps --format-version 1 >"$metadata_file"

node - "$metadata_file" <<'NODE'
const fs = require("fs");
const metadata = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const required = {
  description: "Local-first TUI and reports for AI coding-agent session history, cost, tokens, time, and slow-run diagnosis.",
  repository: "https://github.com/luoyuctl/agenttrace",
  homepage: "https://luoyuctl.github.io/agenttrace/",
  license: "MIT",
  rust_version: "1.80",
};
const expectedKeywords = ["agent", "observability", "tui", "cli", "ai"];
const expectedCategories = ["command-line-utilities", "development-tools"];
const packages = new Map(metadata.packages.map((pkg) => [pkg.name, pkg]));

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

for (const name of ["agenttrace-core", "agenttrace-tui", "agenttrace"]) {
  const pkg = packages.get(name);
  assert(pkg, `missing workspace package ${name}`);
  for (const [field, expected] of Object.entries(required)) {
    assert(pkg[field] === expected, `${name} ${field}=${pkg[field]} expected ${expected}`);
  }
  assert(Array.isArray(pkg.publish) && pkg.publish.length === 0, `${name} should be marked publish=false until crates.io release is intentionally enabled`);
  assert(JSON.stringify(pkg.keywords) === JSON.stringify(expectedKeywords), `${name} keywords drifted`);
  assert(JSON.stringify(pkg.categories) === JSON.stringify(expectedCategories), `${name} categories drifted`);
}

const version = packages.get("agenttrace").version;
for (const [name, pkg] of packages) {
  assert(pkg.version === version, `${name} version ${pkg.version} does not match workspace version ${version}`);
  for (const dep of pkg.dependencies) {
    if (packages.has(dep.name)) {
      assert(dep.req === `^${version}`, `${name} dependency ${dep.name} req ${dep.req} does not match ${version}`);
      assert(dep.path, `${name} dependency ${dep.name} should keep a local path`);
    }
  }
}
NODE

echo "Cargo manifest metadata is aligned."
