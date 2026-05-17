package engine

import "strings"

const (
	ToolAuthorityReadOnlyFiles   = "read_only_files"
	ToolAuthorityWriteFiles      = "write_files"
	ToolAuthorityTestOrBuild     = "test_or_build"
	ToolAuthorityPackageInstall  = "package_install"
	ToolAuthorityShellExec       = "shell_exec"
	ToolAuthorityGitWrite        = "git_write"
	ToolAuthorityNetworkAccess   = "network_access"
	ToolAuthorityExternalPublish = "external_publish"
	ToolAuthorityUnknown         = "unknown_authority"
)

var toolAuthorityRank = map[string]int{
	ToolAuthorityReadOnlyFiles:   1,
	ToolAuthorityTestOrBuild:     2,
	ToolAuthorityWriteFiles:      3,
	ToolAuthorityPackageInstall:  4,
	ToolAuthorityNetworkAccess:   5,
	ToolAuthorityShellExec:       6,
	ToolAuthorityGitWrite:        7,
	ToolAuthorityExternalPublish: 8,
	ToolAuthorityUnknown:         9,
}

func ClassifyToolAuthority(tc ToolCall) string {
	name := strings.ToLower(strings.TrimSpace(tc.Name))
	args := strings.ToLower(strings.TrimSpace(tc.Args))
	combined := strings.TrimSpace(name + " " + args)
	if combined == "" || name == "unknown" {
		return ToolAuthorityUnknown
	}

	switch {
	case containsAny(combined, "npm publish", "pnpm publish", "yarn publish", "twine upload", "docker push", "gh release create", "goreleaser release"):
		return ToolAuthorityExternalPublish
	case containsAny(combined, "git push", "git commit", "git tag", "git merge", "git rebase", "git cherry-pick", "git reset", "git checkout", "git switch"):
		return ToolAuthorityGitWrite
	case containsAny(combined, "npm install", "npm i ", "pnpm install", "pnpm add", "yarn add", "pip install", "uv add", "go get", "cargo add", "brew install"):
		return ToolAuthorityPackageInstall
	case containsAny(combined, "curl ", "wget ", "http://", "https://") || containsAny(name, "web_fetch", "web_search", "network"):
		return ToolAuthorityNetworkAccess
	case containsAny(combined, "go test", "go build", "npm test", "npm run test", "pytest", "cargo test", "cargo build", "make test", "make build", "mvn test", "gradle test"):
		return ToolAuthorityTestOrBuild
	case containsAny(name, "write", "edit", "patch", "delete", "remove", "create_file", "replace"):
		return ToolAuthorityWriteFiles
	case containsAny(name, "bash", "shell", "terminal", "exec", "run_command", "command"):
		return ToolAuthorityShellExec
	case containsAny(name, "go_test", "test", "build"):
		return ToolAuthorityTestOrBuild
	case containsAny(name, "read", "view", "list", "grep", "rg", "find", "cat", "ls"):
		return ToolAuthorityReadOnlyFiles
	default:
		return ToolAuthorityUnknown
	}
}

func HigherToolAuthority(a, b string) string {
	if a == "" {
		return b
	}
	if b == "" {
		return a
	}
	if toolAuthorityRank[b] > toolAuthorityRank[a] {
		return b
	}
	return a
}

func IsHighAuthorityCategory(category string) bool {
	return toolAuthorityRank[category] >= toolAuthorityRank[ToolAuthorityWriteFiles]
}

func containsAny(value string, needles ...string) bool {
	for _, needle := range needles {
		if strings.Contains(value, needle) {
			return true
		}
	}
	return false
}
