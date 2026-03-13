# Commit Message Convention

Applies to all commits in this repository.

## Format

```
type(scope): description
```

## Types

| Type | When to use |
|------|-------------|
| `feat` | New feature or capability |
| `fix` | Bug fix |
| `refactor` | Code restructuring without behavior change |
| `docs` | Documentation only |
| `test` | Adding or updating tests |
| `chore` | Maintenance, CI, dependencies, tooling |

## Scopes

| Scope | Area |
|-------|------|
| `backend` | General backend changes |
| `frontend` | General frontend changes |
| `proxy` | Request proxying logic |
| `streaming` | SSE / AWS Event Stream parsing |
| `auth` | Authentication (API key, OAuth, sessions) |
| `converter` | Format converters (OpenAI/Anthropic/Kiro) |
| `model` | Model resolution, caching, metadata |
| `middleware` | CORS, auth middleware, debug logging |
| `guardrails` | Content safety (CEL rules, Bedrock API) |
| `mcp` | MCP Gateway (client lifecycle, tool execution) |
| `metrics` | Request latency, token tracking |
| `web-ui` | Web UI API handlers, pages, components |
| `config` | Configuration, settings, env vars |
| `docker` | Docker, docker-compose, Dockerfiles |
| `ci` | GitHub Actions, workflows, automation |

## Rules

- **Scope is required** — always include a scope in parentheses
- **Lowercase** — type, scope, and description all lowercase
- **Imperative mood** — "add feature" not "added feature" or "adds feature"
- **Max 72 characters** — total line length including type and scope
- **Reference issues** — use `#N` when the commit closes or relates to a GitHub Issue
- **No period** — do not end the description with a period

## Examples

```
feat(converter): add anthropic tool_use block support
fix(streaming): handle empty event stream chunks
refactor(backend): extract auth middleware into separate module
docs(config): document new guardrails env vars
test(auth): add token refresh edge case coverage
chore(ci): add cargo clippy to PR workflow
```

## Multi-line Messages

For commits that need more detail, add a body separated by a blank line:

```
feat(mcp): add server health monitoring

Poll connected MCP servers every 30s and mark unhealthy servers
as unavailable in the tool registry. Closes #42.
```

## Agent Commits

Commits made by AI agents must include the Co-Authored-By trailer:

```
feat(backend): add guardrails endpoint

Co-Authored-By: Claude <noreply@anthropic.com>
```
