#!/usr/bin/env node
// agenttrace npm runner — delegates to the platform binary
const { spawnSync } = require('child_process');
const path = require('path');
const os = require('os');
const fs = require('fs');

const ext = process.platform === 'win32' ? '.exe' : '';
const binDir = path.join(__dirname, '..', '..', '.bin');

// resolve real binary path (priority: project .bin > local install)
const candidates = [];
if (process.env.AGENTTRACE_BIN) {
    candidates.push(process.env.AGENTTRACE_BIN);
}
candidates.push(path.join(binDir, 'agenttrace' + ext));

// also check /usr/local/bin and ~/.local/bin
const nixCandidates = [
    '/usr/local/bin/agenttrace',
    path.join(os.homedir(), '.local/bin/agenttrace'),
    path.join(os.homedir(), 'bin/agenttrace'),
];
candidates.push(...nixCandidates);

let binary = null;
for (const c of candidates) {
    if (fs.existsSync(c)) {
        binary = c;
        break;
    }
}

if (!binary) {
    console.error('agenttrace binary not found. Install it with one of the current channels:');
    console.error('   brew install luoyuctl/tap/agenttrace');
    console.error('   curl -fsSL https://raw.githubusercontent.com/luoyuctl/agenttrace/master/install.sh | sh');
    process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
process.exit(result.status ?? 1);
