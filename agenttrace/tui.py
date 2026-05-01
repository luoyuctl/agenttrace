#!/usr/bin/env python3
"""
agenttrace TUI — interactive terminal dashboard (zero deps, pure curses).

Views:
  Session List  — scrollable table, sort by column
  Session Detail — full analysis with scroll
  Compare       — multi-session side-by-side

Keys:
  ↑↓ / jk  — navigate
  ←→ / hl  — change sort column
  Enter    — drill into session
  Tab      — switch view
  s        — cycle sort direction
  r        — refresh data
  q        — quit
  ?        — help overlay
"""
import curses
import time
import sys
import os
from pathlib import Path
from collections import Counter

# ── data layer ─────────────────────────────────────────────
try:
    from . import analyze, anomalies, health_score, report_text, report_json
    from . import _find_jsonl_files, detect_format, parse, PRICING
except ImportError:
    from agenttrace import analyze, anomalies, health_score, report_text, report_json
    from agenttrace import _find_jsonl_files, detect_format, parse, PRICING

def _load_session(path):
    """Parse session and return (name, metrics, anomalies_list, health)."""
    name = Path(path).stem
    try:
        fmt = detect_format(path)
        if fmt == "unknown":
            return None
        events = parse(path)
        m = analyze(events)
        a = anomalies(m)
        h = health_score(m, a)
        return (name, m, a, h)
    except Exception:
        return None

def _load_all(directory):
    """Load all sessions from directory, return list of (name, m, a, h)."""
    files = sorted(_find_jsonl_files(Path(directory)))
    results = []
    for f in files:
        r = _load_session(f)
        if r:
            results.append(r)
    return results

# ── drawing helpers ────────────────────────────────────────
def _safe_addstr(win, y, x, text, attr=0, max_w=None):
    """Add string safely, avoiding bottom-right corner bug."""
    h, w = win.getmaxyx()
    if y >= h or x >= w:
        return
    if max_w is None:
        max_w = w - x
    if max_w <= 0:
        return
    text = str(text)[:max_w]
    try:
        win.addstr(y, x, text, attr)
    except curses.error:
        pass

def _hline(win, y, x, w, ch="─", attr=0):
    """Draw horizontal line."""
    for i in range(w):
        _safe_addstr(win, y, x + i, ch, attr)

def _bar(win, y, x, w, pct, colors):
    """Draw a █░ progress bar. pct in 0-1."""
    fills = int(w * pct)
    empty = w - fills
    _safe_addstr(win, y, x, "█" * fills, colors[0])
    _safe_addstr(win, y, x + fills, "░" * empty, colors[1])

def _color_for_health(h):
    """Return color pair index for health score."""
    if h >= 90: return 1   # green
    if h >= 70: return 2   # yellow
    if h >= 50: return 3   # orange
    return 4                # red

def _color_for_rate(r):
    """Return color pair for success rate."""
    if r >= 0.95: return 1
    if r >= 0.80: return 2
    return 4

# ── views ──────────────────────────────────────────────────
class SessionListView:
    """Scrollable table of sessions."""

    def __init__(self):
        self.data = []       # [(name, m, a, h), ...]
        self.selected = 0
        self.scroll = 0
        self.sort_col = 6    # default: health
        self.sort_asc = False

    COLUMNS = [
        ("SESSION",         26, lambda r: r[0][:25]),
        ("TURNS",            6, lambda r: str(r[1].get("assistant_turns", 0))),
        ("TOOLS",            6, lambda r: str(r[1].get("tool_calls_total", 0))),
        ("SUCC",             5, lambda r: f"{r[1].get('tool_calls_ok',0)/max(r[1].get('tool_calls_total',1),1)*100:.0f}%"),
        ("COST",             8, lambda r: f"${r[1].get('cost_estimated',0):.4f}"),
        ("TOKENS",           7, lambda r: str(r[1].get("tokens_input",0)+r[1].get("tokens_output",0))),
        ("HEALTH",           6, lambda r: f"{r[3]}/100"),
    ]

    SORT_KEYS = [
        lambda r: r[0],
        lambda r: r[1].get("assistant_turns", 0),
        lambda r: r[1].get("tool_calls_total", 0),
        lambda r: r[1].get("tool_calls_ok", 0) / max(r[1].get("tool_calls_total", 1), 1),
        lambda r: r[1].get("cost_estimated", 0),
        lambda r: r[1].get("tokens_input", 0) + r[1].get("tokens_output", 0),
        lambda r: r[3],
    ]

    def sort(self):
        self.data.sort(key=self.SORT_KEYS[self.sort_col], reverse=not self.sort_asc)

    def selected_session(self):
        if 0 <= self.selected < len(self.data):
            return self.data[self.selected]
        return None

    def render(self, win, colors):
        h, w = win.getmaxyx()
        win.erase()

        # Title bar
        _safe_addstr(win, 0, 0, " AGENTTRACE — Session Dashboard ", curses.A_REVERSE)
        _safe_addstr(win, 0, w - 30, f" {len(self.data)} sessions ", curses.A_REVERSE)

        # Column headers
        cx = 2
        for ci, (name, cw, _) in enumerate(self.COLUMNS):
            attr = curses.A_BOLD | curses.A_UNDERLINE
            if ci == self.sort_col:
                attr |= curses.color_pair(5)  # cyan for active sort
            arrow = " ▲" if (ci == self.sort_col and not self.sort_asc) else (" ▼" if ci == self.sort_col else "  ")
            _safe_addstr(win, 2, cx, f"{name}{arrow[:2]}", attr)
            cx += cw + 1

        _hline(win, 3, 1, w - 2, "━")

        # Data rows
        visible = h - 6
        if self.selected >= self.scroll + visible:
            self.scroll = self.selected - visible + 1
        if self.selected < self.scroll:
            self.scroll = self.selected

        for i in range(visible):
            di = self.scroll + i
            if di >= len(self.data):
                break
            row_y = 4 + i
            name, m, a, hscore = self.data[di]
            sel = di == self.selected

            cx = 2
            for ci, (cname, cw, cell_fn) in enumerate(self.COLUMNS):
                val = cell_fn(self.data[di])
                attr = curses.A_REVERSE if sel else curses.A_NORMAL
                if ci == 6:  # health column — color
                    attr |= curses.color_pair(_color_for_health(hscore))
                elif ci == 3:  # success rate
                    rate = float(val.strip('%')) / 100
                    attr |= curses.color_pair(_color_for_rate(rate))
                _safe_addstr(win, row_y, cx, val.rjust(cw) if ci > 0 else val.ljust(cw), attr)
                cx += cw + 1

        # Bottom status bar
        status = (
            f" ↑↓ navigate | Enter detail | ←→ sort | s reverse | r refresh | Tab compare | q quit | ? help "
        )
        _safe_addstr(win, h - 1, 0, status[: w - 1], curses.A_REVERSE)

        win.noutrefresh()


class SessionDetailView:
    """Scrollable detail view for one session."""

    def __init__(self):
        self.data = None
        self.scroll = 0
        self.lines = []

    def set_session(self, name, m, a, hscore):
        self.data = (name, m, a, hscore)
        text = report_text(m, a, hscore)
        self.lines = text.split("\n")
        self.scroll = 0

    def render(self, win, colors):
        h, w = win.getmaxyx()
        win.erase()

        if not self.data:
            _safe_addstr(win, h // 2, (w - 20) // 2, "No session selected", curses.A_BOLD)
            win.noutrefresh()
            return

        name = self.data[0]
        _safe_addstr(win, 0, 0, f" AGENTTRACE — {name} ", curses.A_REVERSE)
        _safe_addstr(win, 0, w - 20, " Esc back | ↑↓ scroll ", curses.A_REVERSE)

        visible = h - 2
        for i in range(visible):
            li = self.scroll + i
            if li >= len(self.lines):
                break
            line = self.lines[li]
            # color-code anomaly lines
            attr = curses.A_NORMAL
            if "🔴" in line or "[HIGH]" in line:
                attr = curses.color_pair(4)
            elif "🟡" in line or "[MEDIUM]" in line:
                attr = curses.color_pair(2)
            elif "🟢" in line:
                attr = curses.color_pair(1)
            elif "██" in line or "░░" in line:
                attr = curses.color_pair(1) | curses.A_BOLD
            _safe_addstr(win, i + 1, 1, line, attr)

        _safe_addstr(win, h - 1, 0, f" Line {self.scroll+1}-{min(self.scroll+visible, len(self.lines))}/{len(self.lines)} ", curses.A_REVERSE)
        win.noutrefresh()


class CompareView:
    """Tabular comparison of all sessions."""

    def __init__(self):
        self.sessions = []
        self.scroll = 0

    def set_sessions(self, data):
        self.sessions = data
        self.scroll = 0

    def render(self, win, colors):
        h, w = win.getmaxyx()
        win.erase()

        _safe_addstr(win, 0, 0, " AGENTTRACE — Session Comparison ", curses.A_REVERSE)
        _safe_addstr(win, 0, w - 20, " Tab/` back | ↑↓ scroll ", curses.A_REVERSE)

        if not self.sessions:
            _safe_addstr(win, 3, 2, "No sessions loaded.")
            win.noutrefresh()
            return

        # Headers
        headers = [
            ("Session", 28), ("Turns", 6), ("Tools", 6), ("Succ%", 6),
            ("Fail", 5), ("Cost", 8), ("Tokens", 7), ("Health", 6),
        ]
        splits = [0]
        for _, cw in headers:
            splits.append(splits[-1] + cw + 1)

        cx = 2
        for name, cw in headers:
            _safe_addstr(win, 2, cx, name, curses.A_BOLD | curses.A_UNDERLINE)
            cx += cw + 1

        _hline(win, 3, 1, w - 2, "━")

        visible = h - 5
        for i in range(visible):
            si = self.scroll + i
            if si >= len(self.sessions):
                break
            row_y = 4 + i
            name, m, a, hscore = self.sessions[si]

            cols = [
                name[:27],
                str(m.get("assistant_turns", 0)),
                str(m.get("tool_calls_total", 0)),
                f"{m.get('tool_calls_ok',0)/max(m.get('tool_calls_total',1),1)*100:.0f}%",
                str(m.get("tool_calls_fail", 0)),
                f"${m.get('cost_estimated',0):.4f}",
                str(m.get("tokens_input", 0) + m.get("tokens_output", 0)),
            ]
            cx = 2
            for ci, (cname, cw) in enumerate(headers):
                if ci == len(cols):
                    # health bar
                    pct = hscore / 100
                    bar_w = 6
                    bar_x = cx + (cw - bar_w) // 2
                    _bar(win, row_y, bar_x, bar_w, pct, [
                        curses.color_pair(_color_for_health(hscore)),
                        curses.color_pair(0),
                    ])
                    _safe_addstr(win, row_y, bar_x + bar_w + 1, str(hscore), curses.color_pair(_color_for_health(hscore)))
                else:
                    val = str(cols[ci])
                    if ci == 0:
                        _safe_addstr(win, row_y, cx, val.ljust(cw))
                    else:
                        _safe_addstr(win, row_y, cx, val.rjust(cw))
                cx += cw + 1

        _safe_addstr(win, h - 1, 0, f" {len(self.sessions)} sessions | Scroll {self.scroll+1}-{min(self.scroll+visible, len(self.sessions))} ", curses.A_REVERSE)
        win.noutrefresh()


class HelpOverlay:
    """Popup help screen."""

    def __init__(self):
        self.visible = False

    def toggle(self):
        self.visible = not self.visible

    def render(self, win):
        if not self.visible:
            return
        h, w = win.getmaxyx()
        ph, pw = 18, 50
        py = max(0, (h - ph) // 2)
        px = max(0, (w - pw) // 2)

        popup = curses.newwin(ph, pw, py, px)
        popup.bkgd(' ', curses.color_pair(5) | curses.A_REVERSE)
        popup.box()

        lines = [
            " KEYBINDINGS ",
            "",
            "  ↑ ↓ / j k    Navigate items",
            "  ← → / h l    Change sort column",
            "  Enter        Drill into session detail",
            "  Tab / `      Cycle views (List→Detail→Compare)",
            "  s            Reverse sort direction",
            "  r            Reload sessions from disk",
            "  ?            Toggle this help",
            "  q / Esc      Quit / Go back",
            "",
            " COLORS",
            "  🟢 90+  Green   — Healthy",
            "  🟡 70+  Yellow  — Watch",
            "  🟠 50+  Orange  — Warning",
            "  🔴 <50  Red     — Critical",
            "",
            " Press any key to close",
        ]
        for i, line in enumerate(lines):
            if i < ph - 1:
                _safe_addstr(popup, i + 1, 2, line)

        popup.noutrefresh()
        return popup


# ── TUI controller ─────────────────────────────────────────
class AgentTraceTUI:
    def __init__(self, sessions_dir):
        self.sessions_dir = sessions_dir
        self.views = ["list", "detail", "compare"]
        self.current = "list"
        self.running = True

        self.list_view = SessionListView()
        self.detail_view = SessionDetailView()
        self.compare_view = CompareView()
        self.help_overlay = HelpOverlay()

    def load_data(self):
        data = _load_all(self.sessions_dir)
        self.list_view.data = data
        self.list_view.sort()
        self.compare_view.set_sessions(data)

    def run(self, stdscr):
        # color setup
        curses.start_color()
        curses.use_default_colors()
        curses.curs_set(0)
        curses.init_pair(1, curses.COLOR_GREEN, -1)
        curses.init_pair(2, curses.COLOR_YELLOW, -1)
        curses.init_pair(3, 208, -1)  # orange
        curses.init_pair(4, curses.COLOR_RED, -1)
        curses.init_pair(5, curses.COLOR_CYAN, -1)

        stdscr.clear()
        stdscr.noutrefresh()

        self.load_data()

        # If a session was already selected in list view, auto-open detail
        if self.list_view.data:
            sel = self.list_view.selected_session()
            if sel:
                self.detail_view.set_session(*sel)

        while self.running:
            # ── render current view ──
            if self.current == "list":
                self.list_view.render(stdscr, None)
            elif self.current == "detail":
                self.detail_view.render(stdscr, None)
            elif self.current == "compare":
                self.compare_view.render(stdscr, None)

            if self.help_overlay.visible:
                hp = self.help_overlay.render(stdscr)

            curses.doupdate()

            # ── input ──
            try:
                key = stdscr.getch()
            except KeyboardInterrupt:
                break

            # Help overlay eats keypress to close
            if self.help_overlay.visible and key != -1:
                self.help_overlay.toggle()
                if hp:
                    del hp
                continue

            # ── global keys ──
            if key == ord('q'):
                if self.current == "detail":
                    self.current = "list"
                else:
                    break
            elif key == 27:  # Esc
                if self.current == "detail":
                    self.current = "list"
                elif self.help_overlay.visible:
                    self.help_overlay.toggle()
            elif key == ord('?'):
                self.help_overlay.toggle()
            elif key == ord('r'):
                self.load_data()

            # ── list view keys ──
            elif self.current == "list":
                lv = self.list_view
                if key == ord('\t') or key == ord('`'):
                    self.current = "compare"
                elif key in (curses.KEY_UP, ord('k')):
                    lv.selected = max(0, lv.selected - 1)
                elif key in (curses.KEY_DOWN, ord('j')):
                    lv.selected = min(len(lv.data) - 1, lv.selected + 1)
                elif key in (curses.KEY_LEFT, ord('h')):
                    lv.sort_col = max(0, lv.sort_col - 1)
                    lv.sort()
                elif key in (curses.KEY_RIGHT, ord('l')):
                    lv.sort_col = min(len(lv.COLUMNS) - 1, lv.sort_col + 1)
                    lv.sort()
                elif key == ord('s'):
                    lv.sort_asc = not lv.sort_asc
                    lv.sort()
                elif key in (curses.KEY_ENTER, 10, 13):
                    sel = lv.selected_session()
                    if sel:
                        self.detail_view.set_session(*sel)
                        self.current = "detail"

            # ── detail view keys ──
            elif self.current == "detail":
                dv = self.detail_view
                if key == ord('\t') or key == ord('`'):
                    self.current = "compare"
                elif key in (curses.KEY_UP, ord('k')):
                    dv.scroll = max(0, dv.scroll - 1)
                elif key in (curses.KEY_DOWN, ord('j')):
                    dv.scroll = min(len(dv.lines) - 1, dv.scroll + 1)
                elif key == curses.KEY_NPAGE:
                    dv.scroll = min(len(dv.lines) - 1, dv.scroll + 20)
                elif key == curses.KEY_PPAGE:
                    dv.scroll = max(0, dv.scroll - 20)

            # ── compare view keys ──
            elif self.current == "compare":
                cv = self.compare_view
                if key == ord('\t') or key == ord('`'):
                    self.current = "list"
                elif key in (curses.KEY_UP, ord('k')):
                    cv.scroll = max(0, cv.scroll - 1)
                elif key in (curses.KEY_DOWN, ord('j')):
                    max_scroll = max(0, len(cv.sessions) - 1)
                    cv.scroll = min(max_scroll, cv.scroll + 1)

            # Resize handling
            if key == curses.KEY_RESIZE:
                curses.resize_term(*stdscr.getmaxyx())
                stdscr.clear()


# ── entry point ────────────────────────────────────────────
def main():
    import argparse
    ap = argparse.ArgumentParser(description="agenttrace TUI — interactive session dashboard")
    ap.add_argument("-d", "--dir", default=os.path.expanduser("~/.hermes/sessions"),
                    help="Directory containing session JSONL files")
    args = ap.parse_args()

    if not os.path.isdir(args.dir):
        print(f"Directory not found: {args.dir}")
        sys.exit(1)

    try:
        curses.wrapper(AgentTraceTUI(args.dir).run)
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
