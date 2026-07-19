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

`homebrew/Formula/agenttrace.rb` is the source-tree Formula used for local validation. The public tap is maintained separately at `luoyuctl/homebrew-tap`; it should be updated only after the corresponding GitHub Release assets are available.

Validate the source-tree Formula with:

```bash
ruby -c homebrew/Formula/agenttrace.rb
```

## Future package channels

npm and WinGet publication are not current release guarantees unless their package definitions, credentials, generated manifests, and release-workflow steps are committed together in an approved packaging change.

When adding a channel:

1. Keep package-specific code in a top-level channel directory such as `npm/` or `winget/`.
2. Generate binary URLs and checksums from the published GitHub Release; never duplicate unchecked binaries.
3. Add deterministic local tests and CI coverage for the package contract.
4. Update README install guidance only after the release path is live.
5. Document whether the last step is automated or requires a maintainer-owned external submission.

## Release checks

Run the local Rust release gate before tagging:

```bash
scripts/ci/check-rust-release-local.sh
```

It validates formatting, linting, tests, build output, generated fixtures, report contracts, release surfaces, Pages assets, Homebrew syntax, helper scripts, and real-data smoke paths.

For public-facing changes, follow the protected-surface rules in [AgentOps prompt rules](agentops-prompt-rules.md).
