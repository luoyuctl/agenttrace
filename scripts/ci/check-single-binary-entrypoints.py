#!/usr/bin/env python3
import os
import pty
import select
import subprocess
import sys
import time

binary = os.environ.get("AGENTTRACE_BIN", "target/release/agenttrace")
fixture = "testdata/generated/detailed-tool-steps.jsonl"

cli = subprocess.run(
    [binary, "--sessions", "--limit", "1", fixture],
    text=True,
    capture_output=True,
    timeout=10,
)
if cli.returncode or "SESSION\tHEALTH\tDATA" not in cli.stdout:
    raise SystemExit(f"CLI entrypoint failed: {cli.stderr or cli.stdout}")

pid, fd = pty.fork()
if pid == 0:
    os.execve(binary, [binary, "--demo"], {**os.environ, "TERM": "xterm-256color"})

output = bytearray()
deadline = time.time() + 3
while time.time() < deadline:
    ready, _, _ = select.select([fd], [], [], 0.2)
    if ready:
        try:
            output.extend(os.read(fd, 65536))
        except OSError:
            break
os.write(fd, b"q")

wait_deadline = time.time() + 5
while time.time() < wait_deadline:
    ready, _, _ = select.select([fd], [], [], 0.1)
    if ready:
        try:
            output.extend(os.read(fd, 65536))
        except OSError:
            pass
    done, status = os.waitpid(pid, os.WNOHANG)
    if done:
        break
    time.sleep(0.1)
else:
    os.kill(pid, 9)
    raise SystemExit("TUI entrypoint did not exit after q")
if status != 0:
    raise SystemExit(f"TUI entrypoint failed: status={status}")

print("single binary CLI/TUI entrypoints passed")
