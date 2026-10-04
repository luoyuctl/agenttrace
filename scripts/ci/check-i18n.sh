#!/usr/bin/env bash
# Guards the i18n catalog:
#   1. en.yml and zh-CN.yml define the same keys,
#   2. every literal key passed to tr()/text()/pick()/.t()/label()/Message::new() exists,
#   3. no new inline (en, zh) string pairs sneak back into display code,
#   4. --lang rejects unknown values.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

python3 - <<'PY'
import json, pathlib, re, sys

root = pathlib.Path("crates/agenttrace-core/locales")

def load(path):
    keys, stack = {}, []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        indent = len(line) - len(line.lstrip(" "))
        key, _, value = line.strip().partition(":")
        while stack and stack[-1][0] >= indent:
            stack.pop()
        prefix = ".".join(part for _, part in stack)
        full = f"{prefix}.{key}" if prefix else key
        if value.strip():
            keys[full] = json.loads(value.strip())
        else:
            stack.append((indent, key))
    return keys

en = load(root / "en.yml")
zh = load(root / "zh-CN.yml")
errors = []
for key in sorted(set(en) ^ set(zh)):
    errors.append(f"key {key!r} is missing from {'zh-CN' if key in en else 'en'}.yml")

call = re.compile(
    r'(?:\btr|\btext|\bpick|\.t|\blabel|\bt|Message::new|tr_args)\(\s*(?:[\w.&()]+,\s*)?"([a-z][a-z0-9_]*(?:\.[a-z0-9_]+)+)"'
)
sources = [p for p in pathlib.Path("crates").rglob("*.rs") if "/target/" not in str(p)]
for path in sources:
    text = path.read_text(encoding="utf-8")
    for match in call.finditer(text):
        key = match.group(1)
        if key not in en and not key.startswith(("does.", "msg.nope")):
            line = text.count("\n", 0, match.start()) + 1
            errors.append(f"{path}:{line}: unknown i18n key {key!r}")

pair = re.compile(r'\b(?:text|pick|t|label)\(\s*(?:\w+,\s*)?"[^"]*[A-Za-z][^"]*",\s*"[^"]*[一-鿿][^"]*"\s*\)')
for path in sorted(pathlib.Path("crates").glob("*/src/*.rs")):
    if path.name == "tests.rs":
        continue
    text = path.read_text(encoding="utf-8")
    for match in pair.finditer(text):
        line = text.count("\n", 0, match.start()) + 1
        errors.append(f"{path}:{line}: inline en/zh pair; add a key to locales/*.yml instead")

if errors:
    print("check-i18n failed:", *errors, sep="\n  ", file=sys.stderr)
    sys.exit(1)
print(f"check-i18n: {len(en)} keys, catalogs in sync")
PY

bin="${AGENTTRACE_BIN:-target/release/agenttrace}"
if [ -x "$bin" ]; then
	if "$bin" --demo --overview --lang xx >/dev/null 2>&1; then
		echo "check-i18n: --lang xx should be rejected" >&2
		exit 1
	fi
	"$bin" --demo --overview --lang zh | grep -q "全局概览" \
		|| { echo "check-i18n: --lang zh overview is not localized" >&2; exit 1; }
fi
