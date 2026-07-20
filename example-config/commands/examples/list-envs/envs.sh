#!/bin/sh
# Example arg: context dynamic_list script for Nimble.
# All scripts receive NIMBLE_* environment variables automatically.
# Available: NIMBLE_CONTEXT, NIMBLE_PHRASE, NIMBLE_CONFIG_DIR,
#            NIMBLE_COMMAND_DIR, NIMBLE_OS, NIMBLE_VERSION
#
# arg: context fires this script two ways:
#   - a typed suffix ("list envs acme") is passed as $1, overriding any
#     active context
#   - the bare phrase ("list envs") fires with no $1 as long as a context
#     is active; the project comes from NIMBLE_CONTEXT instead
# With neither a suffix nor an active context, Nimble never runs this
# script at all.
PROJECT="${1:-$NIMBLE_CONTEXT}"

python3 -c "
import json, sys
project = sys.argv[1] if len(sys.argv) > 1 and sys.argv[1] else 'default'
envs = [
    ('staging', 'https://staging.{0}.example.com'),
    ('production', 'https://{0}.example.com'),
    ('sandbox', 'https://sandbox.{0}.example.com'),
]
print(json.dumps([
    {'title': '{0} / {1}'.format(project, name), 'subtext': url.format(project)}
    for name, url in envs
]))
" "$PROJECT"
