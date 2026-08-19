# CodingPet Integration Protocol v1

CodingPet accepts provider-neutral coding-agent events over a local HTTP API. Tool-specific
hooks are adapters: they translate Codex, Claude Code, Cursor, Git, IDE, or CI events into this
stable envelope. The desktop pet never parses provider-specific payloads itself.

## Architecture

```text
provider hook -> thin adapter / `coding-pet emit` -> local HTTP API
                                                    -> validated AgentEvent
                                                    -> winit UI event
                                                    -> pet animation
```

This boundary keeps provider churn outside the animation and window code. A new provider only
needs an adapter that maps its lifecycle names to the canonical event names below.

## Discovery and security

At startup CodingPet binds to a random port on `127.0.0.1` and writes:

```text
%APPDATA%\CodingPet\config\integration.json
```

Example:

```json
{
  "version": 1,
  "pid": 1234,
  "address": "127.0.0.1:49152",
  "token": "64-character-random-token"
}
```

Clients must send the token as `Authorization: Bearer <token>`. CodingPet refuses non-loopback
addresses, accepts only JSON, limits each request to 64 KiB, and never enables browser CORS.
Do not copy `integration.json` into logs or repositories.

## Recommended client: the built-in emitter

The same executable can be called by hooks and scripts:

```powershell
coding-pet emit agent.turn.started --source codex --session turn-123
coding-pet emit agent.input.required --source claude-code --message "Approval needed"
coding-pet emit agent.turn.completed --source cursor --data '{"duration_ms":1200}'
```

Available options are `--source`, `--id`, `--session`, `--cwd`, `--message`, `--timestamp`, and
`--data`. `--data` must be a JSON object. The emitter discovers the port and token automatically,
uses short timeouts, and exits non-zero when delivery fails.

Adapters that already produce the canonical envelope can pipe one event through standard input:

```powershell
Get-Content event.json -Raw | coding-pet.exe emit-json
```

Hook authors should prefer the emitter over embedding HTTP and token-discovery logic in every
integration.

## HTTP API

### Health check

```http
GET /health
```

Returns `200`. It is intended only as a local liveness check.

### Send an event

```http
POST /v1/events
Authorization: Bearer <token>
Content-Type: application/json
```

```json
{
  "version": 1,
  "id": "optional-provider-event-id",
  "source": "codex",
  "event": "agent.turn.started",
  "timestamp": "2026-08-18T18:30:00+08:00",
  "session_id": "turn-123",
  "cwd": "V:\\project\\github\\coding-pet",
  "message": "Working on the task",
  "data": {
    "tool": "shell"
  }
}
```

`version`, `source`, and `event` form the compatibility contract. Unknown top-level JSON fields
are ignored. Provider-specific values belong in `data`. A valid queued event returns `202`.

## Canonical events

| Event | Meaning | Default animation |
|---|---|---|
| `agent.session.started` | Agent session opened | Running |
| `agent.session.ended` | Agent session closed | Idle |
| `agent.turn.started` | A user turn began | Running |
| `agent.turn.completed` | A turn completed successfully | Waving |
| `agent.turn.failed` | A turn failed | Failed |
| `agent.task.started` | Background or delegated task began | Running |
| `agent.task.completed` | Task completed successfully | Waving |
| `agent.task.failed` | Task failed | Failed |
| `agent.tool.started` | Tool execution began | Review |
| `agent.tool.completed` | Tool execution completed | Running |
| `agent.tool.failed` | Tool execution failed | Failed |
| `agent.review.started` | Review or analysis began | Review |
| `agent.input.required` | User response is needed | Waiting |
| `agent.permission.required` | User approval is needed | Waiting |
| `agent.waiting` | Agent is otherwise blocked/waiting | Waiting |
| `agent.idle` | No active work | Idle |

Unknown event names are accepted and logged but do not trigger an animation. This permits a
provider adapter to ship new metadata before CodingPet adds a visual mapping.

## Adapter rules

Each provider adapter should:

1. Translate native lifecycle names into canonical names rather than exposing native names to
   the pet core.
2. Use a stable lowercase `source` such as `codex`, `claude-code`, `cursor`, `vscode`, or `git`.
3. Preserve the provider session/turn ID in `session_id` and native details in `data`.
4. Never include prompts, source code, tool output, secrets, or full environment variables unless
   the user explicitly opts in.
5. Keep hook delivery non-blocking from the provider's perspective. A failed pet notification
   must not fail a coding task.
6. Treat delivery as at-most-once in protocol v1. Retry only connection failures and only with a
   small bounded delay.

Codex integration belongs in a lifecycle hook that invokes the emitter. MCP is a separate,
optional direction for allowing an agent to actively query or control the pet; it is not the
transport for lifecycle notifications.

## Compatibility policy

- Additive fields and new event names do not change `version`.
- Removing or changing field meaning requires a new protocol version and endpoint.
- Adapters should ignore response JSON fields they do not understand.
- CodingPet v1 accepts only event-envelope version `1`.
