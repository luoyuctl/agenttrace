#!/usr/bin/env node
// agenttrace npm install script — downloads platform-specific binary on postinstall

const fs = require('fs');
const path = require('path');
const https = require('https');

const REPO = 'luoyuctl/agenttrace';
const PACKAGE_VERSION = require('./package.json').version;
const RELEASE_TAG = process.env.AGENTTRACE_RELEASE_TAG || `v${PACKAGE_VERSION}`;
const REQUEST_TIMEOUT_MS = Number(process.env.AGENTTRACE_NPM_TIMEOUT_MS || 120000);
const REDIRECT_CODES = new Set([301, 302, 303, 307, 308]);
const REQUEST_OPTIONS = {
    family: 4,
    headers: { 'User-Agent': 'agenttrace-npm-installer' },
};

function getPlatform() {
    const os = process.platform;
    const arch = process.arch;

    let goos, goarch;
    if (os === 'linux') goos = 'linux';
    else if (os === 'darwin') goos = 'darwin';
    else if (os === 'win32') goos = 'windows';
    else throw new Error(`Unsupported OS: ${os}`);

    if (arch === 'x64') goarch = 'amd64';
    else if (arch === 'arm64') goarch = 'arm64';
    else throw new Error(`Unsupported arch: ${arch}`);

    return { goos, goarch, ext: goos === 'windows' ? '.exe' : '' };
}

async function download(url, dest, redirects = 0) {
    return new Promise((resolve, reject) => {
        const tmp = `${dest}.download`;
        const cleanup = () => {
            try { fs.unlinkSync(tmp); } catch (_) { }
        };
        https.get(url, REQUEST_OPTIONS, (res) => {
            if (REDIRECT_CODES.has(res.statusCode)) {
                res.resume();
                if (redirects >= 5 || !res.headers.location) {
                    reject(new Error(`Too many redirects: ${url}`));
                    return;
                }
                const nextUrl = new URL(res.headers.location, url).toString();
                download(nextUrl, dest, redirects + 1).then(resolve, reject);
                return;
            }
            if (res.statusCode !== 200) {
                res.resume();
                cleanup();
                reject(new Error(`HTTP ${res.statusCode}: ${url}`));
                return;
            }

            const file = fs.createWriteStream(tmp);
            res.pipe(file);
            file.on('finish', () => {
                file.close((err) => {
                    if (err) {
                        cleanup();
                        reject(err);
                        return;
                    }
                    fs.renameSync(tmp, dest);
                    resolve();
                });
            });
            file.on('error', (err) => {
                cleanup();
                reject(err);
            });
        }).setTimeout(REQUEST_TIMEOUT_MS, function onTimeout() {
            this.destroy(new Error(`Timed out after ${REQUEST_TIMEOUT_MS}ms: ${url}`));
        }).on('error', (err) => {
            cleanup();
            reject(err);
        });
    });
}

async function fetchRelease() {
    return new Promise((resolve, reject) => {
        const req = https.get({
            hostname: 'api.github.com',
            path: `/repos/${REPO}/releases/tags/${encodeURIComponent(RELEASE_TAG)}`,
            family: 4,
            headers: REQUEST_OPTIONS.headers,
        }, (res) => {
            let body = '';
            res.setEncoding('utf8');
            res.on('data', (chunk) => { body += chunk; });
            res.on('end', () => {
                if (res.statusCode !== 200) {
                    reject(new Error(`GitHub release lookup failed for ${RELEASE_TAG}: HTTP ${res.statusCode}`));
                    return;
                }
                try {
                    resolve(JSON.parse(body));
                } catch (err) {
                    reject(err);
                }
            });
        });
        req.setTimeout(REQUEST_TIMEOUT_MS, () => {
            req.destroy(new Error(`Timed out after ${REQUEST_TIMEOUT_MS}ms: GitHub release lookup for ${RELEASE_TAG}`));
        });
        req.on('error', reject);
    });
}

async function main() {
    const { goos, goarch, ext } = getPlatform();
    const binary = 'agenttrace' + ext;
    const release = await fetchRelease();
    const suffix = `${goos}-${goarch}${ext}`;
    const asset = (release.assets || []).find((item) => item.name === `${binary}-${suffix}` || item.name.endsWith(suffix));
    if (!asset) {
        throw new Error(`No release asset for ${goos}/${goarch} in ${release.tag_name || 'latest release'}`);
    }
    const url = asset.browser_download_url;

    const binDir = path.join(__dirname, 'vendor');
    if (!fs.existsSync(binDir)) fs.mkdirSync(binDir, { recursive: true });

    const dest = path.join(binDir, binary);

    console.log(`📦 agenttrace: downloading for ${goos}/${goarch}...`);
    console.log(`   ${url}`);

    // skip if already installed (check size > 1MB)
    if (fs.existsSync(dest) && fs.statSync(dest).size > 1_000_000) {
        console.log('   ✅ already installed, skipping download');
        fs.chmodSync(dest, 0o755);
        return;
    }

    await download(url, dest);
    fs.chmodSync(dest, 0o755);
    console.log(`   ✅ installed to ${dest}`);
}

main().catch((err) => {
    console.error('❌ agenttrace install failed:', err.message);
    console.log('   Try manual install: brew install luoyuctl/tap/agenttrace');
    console.log('   Or: curl -sL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | sh');
    process.exit(1);
});
