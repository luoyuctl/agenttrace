#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const { existsSync } = require("node:fs");
const { join } = require("node:path");

const executable = join(
	__dirname,
	"..",
	"lib",
	process.platform === "win32" ? "agenttrace.exe" : "agenttrace",
);

if (!existsSync(executable)) {
	console.error(
		"agenttrace could not find its platform binary. Reinstall it with `npm install -g @luoyuctl/agenttrace`.",
	);
	process.exit(1);
}

const result = spawnSync(executable, process.argv.slice(2), {
	stdio: "inherit",
});

if (result.error) {
	console.error(`agenttrace failed to start: ${result.error.message}`);
	process.exit(1);
}

process.exit(result.status ?? 1);
