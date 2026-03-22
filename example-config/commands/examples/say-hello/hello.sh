#!/bin/sh
# Example dynamic_list script for Nimble
# All scripts receive NIMBLE_* environment variables automatically.
# Available: NIMBLE_CONTEXT, NIMBLE_PHRASE, NIMBLE_CONFIG_DIR,
#            NIMBLE_COMMAND_DIR, NIMBLE_OS, NIMBLE_VERSION
QUERY="$1"

if [ -z "$QUERY" ]; then
  echo '[
  {"title":"Hello, World!","subtext":"A classic greeting"},
  {"title":"Hello, Alice","subtext":"alice@example.com"},
  {"title":"Hello, Bob","subtext":"bob@example.com"},
  {"title":"Hello, Carol","subtext":"carol@example.com"}
]'
else
  # Simple case-insensitive filter by query
  python3 -c "
import json, sys
items = [
  {'title': 'Hello, World!', 'subtext': 'A classic greeting'},
  {'title': 'Hello, Alice', 'subtext': 'alice@example.com'},
  {'title': 'Hello, Bob', 'subtext': 'bob@example.com'},
  {'title': 'Hello, Carol', 'subtext': 'carol@example.com'},
]
q = sys.argv[1].lower()
print(json.dumps([i for i in items if q in i['title'].lower() or q in i.get('subtext','').lower()]))
" "$QUERY"
fi
