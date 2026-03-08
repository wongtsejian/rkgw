# ghpages-crt-styling_20260308: Update GH-Pages Styling and Content to Match Frontend CRT Aesthetic

**Type**: feature
**Created**: 2026-03-08
**Preset**: frontend-feature
**Services**: docs

## Problem Statement
The gh-pages documentation site uses a teal/glass-morphism "infrastructure command center" aesthetic that is visually disconnected from the frontend web UI's CRT phosphor terminal theme. Additionally, the documentation content and mermaid diagrams are outdated and no longer reflect the current architecture (multi-provider support, guardrails, MCP gateway, Qwen/Copilot providers, etc.).

## User Story
As a developer visiting the docs site, I want the documentation to visually match the gateway's web UI so that the product feels cohesive and professionally branded.

## Acceptance Criteria
1. Color palette matches frontend variables.css — background (#060609), surfaces, borders, green (#4ade80), cyan (#22d3ee), red/yellow/blue accents
2. Typography uses JetBrains Mono as primary font, monospace throughout, matching font weights and line-height
3. CRT effects applied — scanline overlay, vignette/dot grid, phosphor glow on headings/links/code blocks
4. Component patterns match frontend — sharp corners (2-3px radius), left-border accents on cards, uppercase labels with letter-spacing
5. All documentation pages updated to reflect current project state (multi-provider, guardrails, MCP, Qwen/Copilot providers, etc.)
6. All mermaid diagrams updated to match current architecture and request flows

## Scope Boundaries
**Out of scope:**
- Frontend style modifications (read-only reference)

## Dependencies
None — frontend styles are stable and can be referenced directly.
