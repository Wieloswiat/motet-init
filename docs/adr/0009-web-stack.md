# 0009. SvelteKit, Panda CSS and Ark UI

Status: Provisional

## Context

The web app renders server-side against the AppView, must run under a strict
Content Security Policy (rendered posts are the main untrusted content), and
must let a World restyle the part of the page it governs at runtime.

## Decision

- SvelteKit with adapter-node, Svelte 5 runes, a CSP set in the SvelteKit
  config. vitest runs component tests in a real browser; Playwright runs
  end-to-end tests against a production build.
- `@motet/design` holds the tokens as CSS variables (from the Theme Creator),
  a Panda CSS preset that only names those variables, and contrast checks for
  every theme. A World's theme replaces variables on its subtree; one
  stylesheet serves every World. Panda runs with strict tokens, so a style
  can't bypass the design system without saying so.
- Ark UI for accessible, unstyled components.
- Webfonts are not vendored until each licence is confirmed.

## Revisit when

The first real screens are built. Panda and Ark are the working choice, not
a settled one; replacing them touches `apps/web` and `@motet/design` only.
