# light-mode-toggle_20260307: Add Light Mode UI Support with Dark/Light Toggle

**Type**: feature
**Created**: 2026-03-07
**Preset**: frontend-feature
**Services**: frontend

## Problem Statement

The gateway UI currently only supports a dark CRT terminal aesthetic. Users working in bright environments or who prefer light interfaces have no option to switch. The UI should auto-detect OS theme preference and provide a manual override toggle that persists across sessions.

## User Story

As a gateway user (admin or regular), I want the UI to respect my OS theme preference with a manual dark/light toggle so that I can use the dashboard comfortably in any lighting environment.

## Acceptance Criteria

1. Light mode CSS custom properties defined in `variables.css` with full color scheme
2. All existing components render correctly in both dark and light modes
3. Visible toggle control in the UI (Sidebar)
4. Theme preference saved to `localStorage`, survives page refresh
5. Respects `prefers-color-scheme` media query on first visit; manual toggle overrides OS preference
6. All pages (Login, Profile, Config, Admin, UserDetail, Guardrails, McpClients) visually verified in both modes with no broken layouts or unreadable text

## Scope Boundaries

**Out of scope:**
- No backend changes
- No per-user server-side theme persistence
- No custom theme builder
- No high-contrast/accessibility themes

## Dependencies

None — pure frontend change, builds on existing CSS custom properties in `variables.css`.

## Technical Approach

React context (`ThemeProvider` + `useTheme()` hook) wrapping the app, with `data-theme` attribute on `<html>` for CSS variable switching. OS preference detected via `matchMedia('(prefers-color-scheme: dark)')`. Theme preference persisted to `localStorage`.
