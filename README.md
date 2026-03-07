<div align="center">

# rkgw — Rust Kiro Gateway

**Multi-user proxy gateway for Kiro API (AWS CodeWhisperer)**

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org/)

_A Rust rewrite of [kiro-gateway](https://github.com/jwadow/kiro-gateway) — Use Claude models through any OpenAI or Anthropic compatible tool_

</div>

---

Kiro Gateway sits between your AI client code and the Kiro API. Send requests in OpenAI or Anthropic format — the gateway translates, handles auth, and streams responses back.

## Supported Models

| Model                 | Description                                           |
| --------------------- | ----------------------------------------------------- |
| **Claude Opus 4.6**   | Latest flagship. Adaptive thinking, complex reasoning |
| **Claude Sonnet 4.6** | Latest balanced. Coding, writing, general-purpose     |
| **Claude Haiku 4.5**  | Lightning fast. Quick responses, simple tasks         |

> **Smart Model Resolution:** Use any model name format — `claude-sonnet-4-6`, `claude-sonnet-4.6`, or versioned names like `claude-sonnet-4-20250514`. The gateway normalizes them automatically.

## Features

- OpenAI + Anthropic compatible APIs
- Real-time SSE streaming
- Extended thinking / reasoning
- Multi-user with Google SSO + per-user API keys
- MCP Gateway (external tool servers)
- Content Guardrails (AWS Bedrock)
- Web dashboard with real-time metrics
- Automated TLS via Let's Encrypt
- Proxy-Only Mode (single container, no DB)
- Optional Datadog APM

## Quick Start

```bash
git clone https://github.com/if414013/rkgw.git && cd rkgw
cp .env.example .env  # edit with your domain, Google OAuth creds
./init-certs.sh       # provision TLS
docker compose up -d  # start all services
```

Open `https://your-domain/_ui/` to complete setup.

> For proxy-only mode (no DB/SSO), see the [Quickstart guide](https://if414013.github.io/rkgw/docs/quickstart).

## Documentation

📖 **[Full Documentation](https://if414013.github.io/rkgw/)**

- [Getting Started](https://if414013.github.io/rkgw/docs/getting-started) — Setup walkthrough for both deployment modes
- [Quickstart](https://if414013.github.io/rkgw/docs/quickstart) — Running in under 5 minutes
- [Configuration](https://if414013.github.io/rkgw/docs/configuration) — Environment variables and runtime settings
- [API Reference](https://if414013.github.io/rkgw/docs/api-reference) — Endpoint documentation with examples
- [Architecture](https://if414013.github.io/rkgw/docs/architecture) — System design and component overview
- [Client Setup](https://if414013.github.io/rkgw/docs/client-setup) — Claude Code, Zed, OpenCode, and SDK configs
- [Deployment](https://if414013.github.io/rkgw/docs/deployment) — Production deployment guide
- [Troubleshooting](https://if414013.github.io/rkgw/docs/troubleshooting) — Common issues and solutions

## License

This project is licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0)**.

- You can use, modify, and distribute this software
- **Network use is distribution** — if you run a modified version on a server, you must make the source code available
- Modifications must be released under the same license

See the [LICENSE](LICENSE) file for the full license text.

By submitting a contribution, you agree to the terms of our [Contributor License Agreement (CLA)](CLA.md).

## Disclaimer

This project is not affiliated with, endorsed by, or sponsored by Amazon Web Services (AWS), Anthropic, or Kiro IDE. Use at your own risk and in compliance with the terms of service of the underlying APIs.

---

<div align="center">

**[Back to Top](#rkgw--rust-kiro-gateway)**

</div>
