#!/usr/bin/env bash
# CodingPet Codex hook forwarder — pure relay, no local policy.
#
# All decisions (auto-approve, animations) happen on the CodingPet host.
# This script only forwards stdin to the host and prints the response body.
#
# Usage:
#   coding-pet-hook.sh <HookEventName>     # Codex hook / manual call
#   coding-pet-hook.sh --list              # print supported hooks
#   coding-pet-hook.sh --help
#
# Codex config example (repeat per event you care about):
#   command = [".../coding-pet-hook.sh", "PermissionRequest"]
#
# Manual test (no stdin needed):
#   ./coding-pet-hook.sh PermissionRequest
#   ./coding-pet-hook.sh UserPromptSubmit
#
# Env (optional; defaults are fixed):
#   CODING_PET_URL    default http://127.0.0.1:18765
#   CODING_PET_TOKEN  default coding-pet
#
# Supported HookEventName | pet effect on host
# ----------------------|--------------------
# SessionStart          | Running
# SessionEnd            | Idle
# UserPromptSubmit      | Running
# PreToolUse            | Review
# PostToolUse           | Running
# PermissionRequest     | auto-allow if host enabled, else Waiting
# PreCompact            | Review
# PostCompact           | Running
# SubagentStart         | Running
# SubagentStop          | Waving
# Stop                  | Waving

set -u

CODING_PET_URL="${CODING_PET_URL:-http://127.0.0.1:18765}"
CODING_PET_TOKEN="${CODING_PET_TOKEN:-coding-pet}"

list_hooks() {
  cat <<'EOF'
SessionStart
SessionEnd
UserPromptSubmit
PreToolUse
PostToolUse
PermissionRequest
PreCompact
PostCompact
SubagentStart
SubagentStop
Stop
EOF
}

usage() {
  cat <<EOF
Usage: $(basename "$0") <HookEventName>
       $(basename "$0") --list
       $(basename "$0") --help

Forward a Codex hook payload to CodingPet (${CODING_PET_URL%/}/v1/codex-hook/<HookEventName>).

Supported hooks:
$(list_hooks | sed 's/^/  /')
EOF
}

is_supported_hook() {
  list_hooks | grep -Fxq "$1"
}

HOOK="${1:-}"
case "$HOOK" in
  ""|-h|--help) usage; exit 0 ;;
  --list|list) list_hooks; exit 0 ;;
esac

if ! is_supported_hook "$HOOK"; then
  echo "unknown hook: $HOOK (use --list)" >&2
  exit 1
fi

# Codex pipes JSON on stdin. Manual calls with empty stdin get a minimal body.
payload="$(cat 2>/dev/null || true)"
if [ -z "$payload" ]; then
  payload="{\"hook_event_name\":\"${HOOK}\"}"
fi

# Fail-open: network errors must never block Codex.
printf '%s' "$payload" | curl -fsS -m 2 \
  -H "Authorization: Bearer ${CODING_PET_TOKEN}" \
  -H "Content-Type: application/json" \
  --data-binary @- \
  "${CODING_PET_URL%/}/v1/codex-hook/${HOOK}" \
  || true
