# Document Writer Memory

## Documentation Drift Audit (2026-03-22, post-PR #171 and #172)
- See `drift-audit-findings.md` for consolidated report
- PR #171 fixed most severe drift; PR #172 removed Qwen but didn't update gh-pages
- Remaining: 4 stale, 3 needs update, 9 accurate, 1 low-priority
- Key remaining issues: Gemini refs (2 files), "Kiro only" proxy claims (3 files), Vite→nginx (deployment.md), Google OAuth env vars (2 files)

## Key Source Code Facts
- AppState defined in `backend/src/routes/state.rs` (NOT `routes/mod.rs` as some docs claim)
- ProviderId enum: Kiro, Anthropic, OpenAICodex, Copilot, Custom (5 total — NO Gemini, NO Qwen)
- Qwen and Gemini model names explicitly rejected via `removed_provider_for_model()` in registry.rs
- Providers stored as `providers: ProviderMap` (not individual fields)
- Session cookie: `SameSite=Strict` (not Lax)
- Streaming parser: `SseParser` with text-based JSON extraction (NOT binary AWS Event Stream)
- Google OAuth configured via Web UI admin panel, NOT env vars (no GOOGLE_* in .env.example)
- Password auth: `web_ui/password_auth.rs` with TOTP 2FA, rate limiting
- Default port: 8000 in code; docker-compose overrides to 9999
- Frontend runs Nginx on internal port 80, mapped to host 5173 (NOT Vite dev server)
- Proxy-only mode supports ALL providers via env vars (not Kiro only)

## gh-pages Documentation Structure
- Location: `gh-pages/docs/`
- 16 doc files total
- Jekyll site with `_config.yml`
- Architecture subdirectory: authentication.md, converters.md, converter-routing-summary.md, index.md, request-flow.md, streaming.md
- Top-level: api-reference.md, client-setup.md, configuration.md, deployment.md, getting-started.md, modules.md, quickstart.md, research-notes.md, troubleshooting.md, web-ui.md

## Writing Patterns
- Always verify claims against source code before documenting
- Check `routes/state.rs` for AppState, `providers/types.rs` for ProviderId enum
- Check `docker-compose.yml` AND `config.rs` for port/config — they may differ
- Check `.env.example` for actual env var support (not just what docs claim)
