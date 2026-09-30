# Distribution guide

This guide describes the committed AgentTrace release surfaces and their owners. It is for maintainers; end users should start with the repository [README](../../README.md).

## Source of truth

A `v*` tag triggers [the release workflow](../../.github/workflows/release.yml). It builds and publishes signed-by-checksum GitHub Release assets for:

- Linux AMD64 and ARM64
- macOS Intel and Apple Silicon
- Windows AMD64 and ARM64

The release workflow also creates `checksums.txt` and provenance attestations. The GitHub Release is the source for the shell and PowerShell installers:

```text
install.sh
install.ps1
```

These scripts remain at repository root because users invoke them through stable raw-GitHub URLs.

## Homebrew

The checked-in `homebrew/Formula/agenttrace.rb` is a `HEAD` Formula used only for local validation. The release workflow generates the versioned, checksum-pinned Formula from the tag and GitHub Release assets, then publishes it to `luoyuctl/homebrew-tap`.

npm and WinGet are also release channels:

- The workflow sets the npm package version from the `v*` tag immediately before packing and publishing it. Its postinstall hook downloads the matching checksummed GitHub Release binary.
- The workflow renders Homebrew and WinGet metadata from the same `checksums.txt` artifact.
- WinGet submits `Luoyuctl.AgentTrace` through `winget-create`.

The source tree deliberately uses non-release version placeholders. A release tag is the only source of a public version, so package metadata and rendered manifests never need manual version bumps.

## Recovering a partial release

If a release published the GitHub Release and npm package but a later channel
step failed (for example an expired `HOMEBREW_TAP_TOKEN`), do not re-run the
release job: it would try to recreate the existing release and npm version.
Fix the secret, then run the **Publish package channels** workflow for the same
tag. It re-renders the Homebrew Formula and WinGet manifests from that release's
`checksums.txt` and publishes only the channels you select.

```bash
gh workflow run publish-channels.yml -f tag=v0.9.0
```

## Release checks

Run the local Rust release gate before tagging:

```bash
scripts/ci/check-rust-release-local.sh
```

It validates formatting, linting, tests, build output, parser fixtures, report contracts, release surfaces, Homebrew syntax, helper scripts, and real-data smoke paths.

For public-facing changes, follow the protected-surface rules in [AgentOps prompt rules](agentops-prompt-rules.md).
