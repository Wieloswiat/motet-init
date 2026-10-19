# 0008. Typst in a sandboxed worker; static SVG until typst.ts

Status: Accepted

## Context

Posts are written in Typst, which is untrusted input to a full typesetting
system: it can loop, allocate, and load WebAssembly plugins. The backend plan
targets typst.ts's vector artifacts drawn in the browser, which follows a
patched Typst whose versions lag the upstream compiler.

## Decision

- `motet-render` compiles in a separate process (`motet-render-worker`)
  started with a clean environment, address-space and CPU rlimits, a
  wall-clock timeout and an output cap. Those limits, not a package denylist,
  are the execution boundary.
- The compiler's world sees only the post's source, bundled fonts and
  Motet's package mirror, with the date fixed to the post's own. No file or
  network access.
- Output is cached under a `RenderKey` naming the source, compiler version,
  security-policy version and layout, so an upgrade re-renders instead of
  serving stale output.
- For now the worker produces static SVG with upstream `typst`. Moving to
  typst.ts vector artifacts replaces the output format and pins the worker's
  Typst to typst.ts's; the sandbox and supervisor stay.

## Revisit when

The typst.ts spike in Groundwork (two widths, both themes, a screen reader)
is done.
