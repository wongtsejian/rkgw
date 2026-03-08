# ghpages-crt-styling_20260308: Implementation Plan

**Status**: draft
**Branch**: feat/ghpages-crt-styling
**Parallelism**: 3 react-frontend-engineers + 1 frontend-qa

## Wave 1 (Parallel — all 3 agents start simultaneously)

### Agent: fe-styling (react-frontend-engineer)
**File ownership**: `_sass/custom/custom.scss`, `index.md`

- [ ] 1.1 — Rewrite `_sass/custom/custom.scss` color palette: replace teal/blue/amber with frontend's green (#4ade80), cyan (#22d3ee), red (#f87171), yellow (#fbbf24), blue (#60a5fa). Dark backgrounds (#060609, #0b0b10, #101018), borders (#1a1a26), text (#b8ccb8, #7a8e7a)
- [ ] 1.2 — Add JetBrains Mono font import and apply as primary font throughout. Set line-height 1.5, font weights 400/500/600/700
- [ ] 1.3 — Add CRT effects: scanline overlay (::before pseudo-element, 2-4px repeating gradient), vignette (radial gradient edge darkening), dot grid pattern, phosphor glow on headings/links/code (box-shadow with rgba(74,222,128))
- [ ] 1.4 — Update component patterns: sharp borders (2-3px radius), left-border accents on feature/nav cards, uppercase labels with letter-spacing, hover glow instead of lift
- [ ] 1.5 — Style code blocks, tables, blockquotes, and inline code to match frontend patterns (green-tinted backgrounds, sharp corners, monospace)
- [ ] 1.6 — Update `index.md`: hero section styling (gradient text white→green), feature cards, nav cards to CRT style, AND update content to reflect current features (multi-provider, guardrails, MCP gateway, Qwen/Copilot providers)

### Agent: fe-docs-arch (react-frontend-engineer)
**File ownership**: `docs/architecture/index.md`, `docs/architecture/request-flow.md`, `docs/architecture/authentication.md`, `docs/architecture/converters.md`, `docs/architecture/streaming.md`, `docs/modules.md`

- [ ] 2.1 — Update `docs/architecture/index.md` and `docs/architecture/request-flow.md`: update mermaid diagrams for current request flow including provider routing, guardrails, MCP
- [ ] 2.2 — Update `docs/architecture/authentication.md`: add multi-provider OAuth (Copilot, Qwen device flow), per-user Kiro token management, update mermaid diagrams
- [ ] 2.3 — Update `docs/architecture/converters.md` and `docs/architecture/streaming.md`: reflect current converter architecture and streaming parser
- [ ] 2.4 — Update `docs/modules.md`: add guardrails/, mcp/, providers/ modules, update existing module descriptions

### Agent: fe-docs-ops (react-frontend-engineer)
**File ownership**: `docs/api-reference.md`, `docs/web-ui.md`, `docs/configuration.md`, `docs/deployment.md`, `docs/getting-started.md`, `docs/quickstart.md`, `docs/client-setup.md`, `docs/troubleshooting.md`, `docs/research-notes.md`

- [ ] 3.1 — Update `docs/api-reference.md`: add MCP endpoints, guardrails endpoints, provider OAuth endpoints
- [ ] 3.2 — Update `docs/web-ui.md`: reflect current admin pages (MCP clients, guardrails, provider management, user management)
- [ ] 3.3 — Update `docs/configuration.md` and `docs/deployment.md`: add proxy-only mode, provider env vars, guardrails/MCP config
- [ ] 3.4 — Update `docs/getting-started.md`, `docs/quickstart.md`, `docs/client-setup.md`: reflect current setup flow and multi-provider support
- [ ] 3.5 — Update `docs/troubleshooting.md` and `docs/research-notes.md`: add provider-specific troubleshooting, update research notes

## Wave 2 (After Wave 1 completes)

### Agent: frontend-qa
**File ownership**: read-only (screenshots to `.playwright-mcp/`)

- [ ] 4.1 — Visual verification: screenshot all pages, verify CRT aesthetic consistency across index, docs, architecture subpages
- [ ] 4.2 — Verify responsive behavior at 800px and 500px breakpoints
- [ ] 4.3 — Verify mermaid diagrams render correctly with new color scheme
