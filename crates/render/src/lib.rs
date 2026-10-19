//! Typst rendering, as a worker process.
//!
//! Post bodies are Typst source written by anyone, and Typst is a
//! programming language (with WebAssembly plugins loaded from raw bytes), so
//! rendering runs in a separate process whose limits are the execution
//! boundary, not any check on the source:
//!
//! - [`world`]: what a post can reach while compiling. Its own source, the
//!   bundled fonts, packages from Motet's mirror, and a fixed date. No other
//!   files, no network, no environment.
//! - [`job`]: the request and response, and the [`job::RenderKey`] a render
//!   is cached under (compiler, renderer and policy versions included).
//! - [`worker`]: the child's side: apply OS limits, read one job, compile,
//!   answer, exit.
//! - [`supervisor`]: the parent's side: spawn a worker per job, enforce wall
//!   clock and output size, turn crashes into errors.
//!
//! Output today is static SVG, what readers without JavaScript and
//! independent clients get. The typst.ts vector artifact (drawn in the
//! browser at the reader's width and theme) is added beside it once the
//! Groundwork spike pins typst.ts, its Typst and the package set together;
//! see `docs/adr/0008-rendering.md`.

pub mod job;
pub mod supervisor;
pub mod worker;
pub mod world;
