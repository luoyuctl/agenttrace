## Summary

What changed and why?

## Coordination

- Linked issue:
- Protected public surface touched? no
- Parser PR mergeability checked after latest `master`? n/a

## Validation

- [ ] `test -z "$(gofmt -l .)"`
- [ ] `go vet ./...`
- [ ] `go test ./...`
- [ ] `go build ./cmd/agenttrace`
- [ ] `scripts/ci/check-output-contract.sh`
- [ ] `scripts/ci/check-deterministic-output.sh`
- [ ] `scripts/ci/check-report-semantics.sh`
- [ ] `scripts/ci/check-release-surfaces.sh`
- [ ] `scripts/ci/check-docs-commands.sh`
- [ ] `scripts/ci/check-pages-artifact.sh site`
- [ ] `node --check npm/install.js && node --check npm/run.js`
- [ ] `ruby -c homebrew/Formula/agenttrace.rb`

## Notes

Mention any parser fixtures, screenshots, or privacy-sensitive test data that were intentionally omitted.
