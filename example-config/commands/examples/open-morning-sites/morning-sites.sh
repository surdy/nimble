#!/bin/sh
# Example script_action script for Ctx Launcher
# Returns a JSON array of strings (URLs).
# Used with result_action: open_url to open each URL in the default browser.
echo '[
  "https://github.com",
  "https://news.ycombinator.com",
  "https://www.reddit.com"
]'
