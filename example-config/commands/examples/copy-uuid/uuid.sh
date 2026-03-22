#!/bin/sh
# Example script_action script for Ctx Launcher
# Generates a random UUID and outputs it as plain text.
# Used with result_action: copy_text to copy the UUID to the clipboard.
uuidgen | tr '[:upper:]' '[:lower:]'
