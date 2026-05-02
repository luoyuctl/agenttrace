#!/usr/bin/env node
// agenttrace npm install script — downloads platform-specific binary on postinstall

const fs = require('fs');
const path = require('path');
const https = require('https');

const REPO = 'luoyuctl/agenttrace';

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

async function download(url, dest) {
    return new Promise((resolve, reject) => {
        const file = fs.createWriteStream(dest);
        https.get(url, (res) => {
            if (res.statusCode === 302 || res.statusCode === 301) {
                // follow redirect
                https.get(res.headers.location, (r2) => {
                    r2.pipe(file);
                    file.on('finish', () => { file.close(); resolve(); });
                }).on('error', reject);
                return;
            }
            if (res.statusCode !== 200) {
                file.close();
                fs.unlinkSync(dest);
                reject(new Error(`HTTP ${res.statusCode}: ${url}`));
                return;
            }
            res.pipe(file);
            file.on('finish', () => { file.close(); resolve(); });
        }).on('error', reject);
    });
}

async function fetchLatestRelease() {
    return new Promise((resolve, reject) => {
        const req = https.get({
            hostname: 'api.github.com',
            path: `/repos/${REPO}/releases/latest`,
            headers: { 'User-Agent': 'agenttrace-npm-installer' },
        }, (res) => {
            let body = '';
            res.setEncoding('utf8');
            res.on('data', (chunk) => { body += chunk; });
            res.on('end', () => {
                if (res.statusCode !== 200) {
                    reject(new Error(`GitHub release lookup failed: HTTP ${res.statusCode}`));
                    return;
                }
                try {
                    resolve(JSON.parse(body));
                } catch (err) {
                    reject(err);
                }
            });
        });
        req.on('error', reject);
    });
}

async function main() {
    const { goos, goarch, ext } = getPlatform();
    const binary = 'agenttrace' + ext;
    const release = await fetchLatestRelease();
    const suffix = `${goos}-${goarch}${ext}`;
    const asset = (release.assets || []).find((item) => item.name === `${binary}-${suffix}` || item.name.endsWith(suffix));
    if (!asset) {
        throw new Error(`No release asset for ${goos}/${goarch} in ${release.tag_name || 'latest release'}`);
    }
    const url = asset.browser_download_url;

    // install to node_modules/.bin/ sibling
    const binDir = path.join(__dirname, '..', '..', '.bin');
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
    // don't fail the npm install — user can still install manually
});
