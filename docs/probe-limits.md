# Probing Model Limits

`probe_limits` is a binary that empirically tests the context window and output token limits for each model supported by the gateway. Use it to determine the correct values for your OpenCode provider config.

## Prerequisites

The gateway must be running locally before you run this tool.

## Usage

```bash
# Probe a single model
cargo run --bin probe_limits --release -- --model claude-sonnet-4.6

# Probe all claude-* models
cargo run --bin probe_limits --release -- --all-models
```

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `PROXY_API_KEY` | *(required)* | Gateway API key |
| `GATEWAY_URL` | `http://127.0.0.1:8000` | Gateway base URL |

These are read from `.env` automatically if present.

## Output

```
Gateway: http://127.0.0.1:8000
Probing model: claude-sonnet-4.6

Model                          Context (tokens)       Output cap
------------------------------------------------------------------
claude-sonnet-4.6                        ~197928   model stops early
```

**Context (tokens)** — the highest `prompt_tokens` value that succeeded, read directly from the gateway's usage metadata. Use this for `contextLength` in your OpenCode config.

**Output cap** — if the model hit `finish_reason=length`, shows the actual `completion_tokens` at the cap. If the model always stops before hitting the limit (common with thinking mode enabled), shows `model stops early`.

## OpenCode Config

Map the results to your provider's `models` block:

```json
"claude-sonnet-4.6": {
  "name": "Claude Sonnet 4.6",
  "limit": {
    "context": 198000,
    "output": 8192
  }
}
```

## Notes

- **Thinking mode**: If the gateway has `FAKE_REASONING=true` (default), thinking tokens consume `max_tokens` budget, making output cap detection unreliable. Restart with `FAKE_REASONING=false` before probing output limits.
- **Context probe accuracy**: The binary search uses character count as a proxy for tokens (~4 chars/token). The reported token count comes from the gateway's tiktoken estimate, not Kiro's tokenizer directly.
- **`auto` model**: Skipped by default since it's a routing alias, not a real model with its own limits.
