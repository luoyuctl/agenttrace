# TUI Benchmark Gap

This note tracks the gap between agenttrace and production-grade TUIs such as lazygit, k9s, btop, htop, Textual apps, and modern terminal AI coding tools.

## Current Position

agenttrace is already past a basic CLI dashboard. It has responsive layouts, an overview/list/detail/diagnostics/diff flow, command mode, filters, cache-aware loading, and machine-readable exits.

Against benchmark TUIs, the remaining gap is mostly product UX, not framework capability. Bubble Tea is still a good fit; replacing it would not by itself close the gap.

## Scorecard

| Area | Benchmark Pattern | agenttrace Status | Gap |
|---|---|---|---|
| First-screen triage | One screen shows the most urgent thing and the next action | Improved with `TRIAGE NOW` and `Next` lanes | Low |
| Stable navigation | Vim-style movement, persistent key footer, modal help | Present | Low |
| Command palette | Fuzzy searchable commands and contextual actions | Command mode exists, no fuzzy palette yet | Medium |
| Progressive disclosure | Summary first, deep diagnostics on demand | Present across overview/detail/diagnostics/diff | Low |
| Empty/loading states | Actionable empty states and visible progress | Present | Low |
| Layout density | Fixed mental map, responsive panels, dense tables | Present, but no user density presets | Medium |
| Accessibility | Semantic color plus non-color labels, theme options | Mostly semantic colors; limited theme/contrast controls | Medium |
| Performance feedback | Loading progress, cache state, background work visibility | Present; startup still benefits from cache | Low to medium |
| Guided remediation | Issue, impact, evidence, exact next step | Improved in detail and overview triage | Low |

## References

- lazygit overview and keybindings: https://bwplotka.dev/2025/lazygit/
- lazygit keybinding docs: https://github.com/jesseduffield/lazygit/blob/master/docs/keybindings/Keybindings_en.md
- k9s commands and hotkeys: https://k9scli.io/topics/commands/ and https://k9scli.io/topics/hotkeys/
- btop dashboard presets and live graphs: https://cubiclenate.com/2024/04/19/btop-terminal-based-resource-monitor/
- htop header/list/footer structure: https://linuxize.com/post/htop-command-in-linux/
- Textual command palette: https://textual.textualize.io/guide/command_palette/
- NN/g empty-state guidance: https://www.nngroup.com/articles/empty-state-interface-design/

## Recommended Next Bets

1. Add fuzzy command search over command mode actions.
2. Add density/theme presets for wide terminals, small terminals, and high-contrast use.
3. Add a keyboard-driven issue workflow: open triage item, apply filter, jump to evidence, export report.
4. Add snapshot tests for representative terminal sizes so layout regressions are caught before release.
