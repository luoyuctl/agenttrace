#!/usr/bin/env node

const {
	chmodSync,
	createWriteStream,
	existsSync,
	mkdirSync,
	readFileSync,
	renameSync,
	rmSync,
} = require("node:fs");
const { arch, platform } = require("node:process");
const { join } = require("node:path");
const { pipeline } = require("node:stream/promises");
const { get } = require("node:https");
const { createHash } = require("node:crypto");

const PACKAGE_VERSION = require("../package.json").version;
const REPOSITORY = "luoyuctl/agenttrace";
const ARCHITECTURES = { x64: "amd64", arm64: "arm64" };
const PLATFORMS = { darwin: "darwin", linux: "linux", win32: "windows" };

function fail(message) {
	throw new Error(`agenttrace installation failed: ${message}`);
}

function getAssetName() {
	const targetPlatform = PLATFORMS[platform];
	const targetArchitecture = ARCHITECTURES[arch];
	if (!targetPlatform || !targetArchitecture) {
		fail(`unsupported platform: ${platform}/${arch}`);
	}

	return `agenttrace-${targetPlatform}-${targetArchitecture}${
		platform === "win32" ? ".exe" : ""
	}`;
}

function download(url, destination, redirects = 0) {
	return new Promise((resolve, reject) => {
		get(
			url,
			{
				headers: {
					Accept: "application/octet-stream",
					"User-Agent": "agenttrace-npm-installer",
				},
			},
			(response) => {
				const location = response.headers.location;
				if (
					response.statusCode >= 300 &&
					response.statusCode < 400 &&
					location &&
					redirects < 5
				) {
					response.resume();
					resolve(download(new URL(location, url), destination, redirects + 1));
					return;
				}
				if (response.statusCode !== 200) {
					response.resume();
					reject(new Error(`download returned HTTP ${response.statusCode}`));
					return;
				}
				pipeline(response, createWriteStream(destination)).then(
					resolve,
					reject,
				);
			},
		).on("error", reject);
	});
}

async function main() {
	const asset = getAssetName();
	const releaseUrl = `https://github.com/${REPOSITORY}/releases/download/v${PACKAGE_VERSION}/${asset}`;
	const checksumUrl = `${releaseUrl}.sha256`;
	const packageRoot = join(__dirname, "..");
	const libDirectory = join(packageRoot, "lib");
	const temporaryBinary = join(libDirectory, `${asset}.download`);
	const temporaryChecksum = join(libDirectory, `${asset}.sha256`);
	const destination = join(
		libDirectory,
		platform === "win32" ? "agenttrace.exe" : "agenttrace",
	);

	mkdirSync(libDirectory, { recursive: true });
	rmSync(temporaryBinary, { force: true });
	rmSync(temporaryChecksum, { force: true });

	try {
		await Promise.all([
			download(releaseUrl, temporaryBinary),
			download(checksumUrl, temporaryChecksum),
		]);
		const expected = readFileSync(temporaryChecksum, "utf8")
			.trim()
			.split(/\s+/)[0];
		const actual = createHash("sha256")
			.update(readFileSync(temporaryBinary))
			.digest("hex");
		if (expected !== actual) {
			fail(`checksum mismatch for ${asset}`);
		}
		renameSync(temporaryBinary, destination);
		if (platform !== "win32") {
			chmodSync(destination, 0o755);
		}
	} finally {
		rmSync(temporaryBinary, { force: true });
		rmSync(temporaryChecksum, { force: true });
	}

	if (!existsSync(destination)) {
		fail(`downloaded binary was not installed: ${asset}`);
	}
}

main().catch((error) => {
	console.error(error.message);
	process.exit(1);
});
