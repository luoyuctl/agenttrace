#!/usr/bin/env node
// agentwaste npm runner — delegates to the platform binary
const { spawnSync } = require('child_process');
const path = require('path');
const os = require('os');
const fs = require('fs');

const ext = process.platform === 'win32' ? '.exe' : '';
const binDir = path.join(__dirname, '..', '..', '.bin');

// resolve real binary path (priority: project .bin > local install)
const candidates = [];
if (process.env.AGENTRACE_BIN) {
    candidates.push(process.env.AGENTRACE_BIN);
}
candidates.push(path.join(binDir, 'agentwaste' + ext));

// also check /usr/local/bin and ~/.local/bin
const nixCandidates = [
    '/usr/local/bin/agentwaste',
    path.join(os.homedir(), '.local/bin/agentwaste'),
    path.join(os.homedir(), 'bin/agentwaste'),
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
    console.error('❌ agentwaste binary not found. Install it:');
    console.error('   npm install -g agentwaste');
    console.error('   brew install luoyuctl/tap/agentwaste');
    console.error('   curl -sL https://raw.githubusercontent.com/luoyuctl/agentwaste/master/install.sh | sh');
    process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
process.exit(result.status ?? 1);
