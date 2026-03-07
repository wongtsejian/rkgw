# light-mode-toggle_20260307: Implementation Plan

**Status**: complete
**Branch**: feat/light-mode-toggle_20260307

## Phase 1: Frontend
Agent: react-frontend-engineer

- [x] 1.1 — Define light mode CSS custom properties in `variables.css` using `[data-theme="light"]` selector (backgrounds, surfaces, borders, text colors, accents, glows)
- [x] 1.2 — Create `ThemeProvider` context and `useTheme()` hook in `src/lib/theme.tsx` (OS preference detection via `matchMedia`, localStorage persistence, `data-theme` attribute on `<html>`)
- [x] 1.3 — Create `ThemeToggle` component in `src/components/ThemeToggle.tsx` (dark/light icon toggle button)
- [x] 1.4 — Integrate `ThemeProvider` in `App.tsx` (wrap app) and add `ThemeToggle` to `Sidebar.tsx`
- [x] 1.5 — Audit and fix `global.css` and `components.css` for hardcoded colors — replace with CSS custom properties
- [x] 1.6 — Audit all 7 pages and 10 components for hardcoded colors or dark-mode-only styles; fix any issues

## Phase 2: QA
Agent: frontend-qa

- [x] 2.1 — Visual verification via Playwright: dark mode, light mode, and dark-restored screenshots all pass (login page)
- [x] 2.2 — Toggle persistence verified: localStorage survives navigation, theme attribute correctly restored
- [x] 2.3 — `npm run lint` and `npm run build` pass (no new errors; 5 pre-existing lint issues)
