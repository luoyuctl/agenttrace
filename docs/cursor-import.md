# Cursor Import

Cursor stores local chat and composer state in SQLite `state.vscdb` files under the VS Code-style workspace storage directory. `agenttrace` keeps the main binary pure Go and reads exported JSON instead of opening SQLite directly.

## Export a Workspace

macOS default path:

```bash
db="$HOME/Library/Application Support/Cursor/User/workspaceStorage/<workspace-id>/state.vscdb"
sqlite3 "$db" "
select json_group_object(key, json(value))
from ItemTable
where key in (
  'aiService.prompts',
  'aiService.generations',
  'composer.composerData'
);
" > cursor-export.json
```

Then run:

```bash
agenttrace cursor-export.json
agenttrace --overview -d .
```

## Supported Cursor Shapes

- `aiService.prompts`: user prompt entries
- `aiService.generations`: assistant generation metadata with timestamps
- `composer.composerData`: composer/session metadata

The export intentionally avoids Cursor auth tokens and unrelated UI state. If Cursor changes the internal schema, export the relevant key values as JSON and keep the original key names above.
