const assert = require("node:assert/strict");
const { existsSync, readFileSync } = require("node:fs");
const { join } = require("node:path");
const test = require("node:test");

const packageRoot = join(__dirname, "..");
const manifest = require(join(packageRoot, "package.json"));

test("publishes one agenttrace command backed by the native installer", () => {
	assert.equal(manifest.name, "@zack78/agenttrace");
	assert.equal(manifest.bin.agenttrace, "bin/agenttrace.js");
	assert.equal(manifest.scripts.postinstall, "node scripts/install.js");
	assert.equal(manifest.publishConfig.access, "public");
	assert.ok(existsSync(join(packageRoot, manifest.bin.agenttrace)));

	const installer = readFileSync(
		join(packageRoot, "scripts", "install.js"),
		"utf8",
	);
	assert.match(installer, /releases\/download\/v\$\{PACKAGE_VERSION\}/);
	assert.match(installer, /checksum mismatch/);
	assert.match(installer, /darwin/);
	assert.match(installer, /linux/);
	assert.match(installer, /win32/);

	const releaseWorkflow = readFileSync(
		join(packageRoot, "..", ".github", "workflows", "release.yml"),
		"utf8",
	);
	assert.match(
		releaseWorkflow,
		/npm config set \/\/registry\.npmjs\.org\/:_authToken/,
	);
	assert.match(releaseWorkflow, /npm publish "\.\/dist\/zack78-agenttrace-/);
});
