#!/bin/sh
# Demonstrates the three user-defined env layers:
#   1. Global env.yaml  → WORK_EMAIL, TEAM (overridden by sidecar)
#   2. Sidecar env.yaml → TEAM (overrides global), PROJECT
#   3. Inline env:      → INLINE_VAR
cat <<EOF
[
  {"title": "WORK_EMAIL (global)",   "subtext": "${WORK_EMAIL:-<not set>}"},
  {"title": "TEAM (sidecar > global)", "subtext": "${TEAM:-<not set>}"},
  {"title": "PROJECT (sidecar)",     "subtext": "${PROJECT:-<not set>}"},
  {"title": "INLINE_VAR (inline)",   "subtext": "${INLINE_VAR:-<not set>}"}
]
EOF
