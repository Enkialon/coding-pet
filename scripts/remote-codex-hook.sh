#!/usr/bin/env bash
# Deprecated alias — use coding-pet-hook.sh <HookEventName>.
exec "$(cd "$(dirname "$0")" && pwd)/coding-pet-hook.sh" "$@"
