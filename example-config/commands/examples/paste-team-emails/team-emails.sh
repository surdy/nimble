#!/bin/sh
# Example script_action script for Ctx Launcher
# Returns a JSON array of strings (email addresses).
# Used with result_action: paste_text and suffix: "\n" to paste each address on its own line.
echo '[
  "alice@example.com",
  "bob@example.com",
  "carol@example.com"
]'
