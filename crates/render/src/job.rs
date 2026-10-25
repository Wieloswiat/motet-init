//! What goes to a worker and what comes back.

use serde::{Deserialize, Serialize};

/// Bump when anything that affects output changes without the compiler's or
/// renderer's version changing: the sandbox, fonts, limits that truncate.
pub const POLICY_VERSION: u32 = 1;

/// One post version to render.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    /// Typst source, as it is in the record.
    pub source: String,
    /// The date `datetime.today()` sees: the post's own `createdAt`, so a
    /// render never depends on when it happened.
    pub date: (i32, u8, u8),
    pub limits: Limits,
}

/// Starting limits from the backend plan, for all of a post's layouts
/// together. To be tuned with measurements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[expect(
    clippy::struct_field_names,
    reason = "max_ distinguishes upper resource bounds and preserves the job wire format"
)]
pub struct Limits {
    pub max_source_bytes: usize,
    pub max_memory_bytes: u64,
    pub max_cpu_seconds: u64,
    pub max_wall_millis: u64,
    pub max_output_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024,
            max_memory_bytes: 512 * 1024 * 1024,
            max_cpu_seconds: 2,
            max_wall_millis: 5_000,
            max_output_bytes: 8 * 1024 * 1024,
        }
    }
}

/// A successful render.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rendered {
    /// One SVG per page.
    pub pages: Vec<String>,
    pub warnings: Vec<String>,
}

/// Why a render failed. Deterministic failures (the source is wrong) are
/// cached like successes; the others may succeed on another try.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum Failure {
    #[error("the source doesn't compile: {}", .0.join("; "))]
    Source(Vec<String>),
    #[error("the source is over {0} bytes")]
    TooLong(usize),
    #[error("the output is over {0} bytes")]
    TooLarge(usize),
    #[error("rendering took too long")]
    Timeout,
    #[error("the worker ran out of memory or crashed")]
    Crashed,
}

impl Failure {
    /// Whether the same job will fail the same way every time.
    #[must_use]
    pub fn is_deterministic(&self) -> bool {
        matches!(self, Self::Source(_) | Self::TooLong(_) | Self::TooLarge(_))
    }
}

pub type Outcome = Result<Rendered, Failure>;

/// What a render is cached under. Artifacts are only valid for the exact
/// versions that produced them, so a compiler upgrade means re-rendering
/// from source, never reusing old output.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RenderKey {
    /// The post version's CID: the source is part of the record.
    pub cid: String,
    pub compiler: String,
    pub policy: u32,
}

impl RenderKey {
    pub fn new(cid: impl Into<String>) -> Self {
        Self {
            cid: cid.into(),
            compiler: compiler_version(),
            policy: POLICY_VERSION,
        }
    }
}

/// The Typst version this worker compiles with.
#[must_use]
pub fn compiler_version() -> String {
    format!("typst {}", typst::utils::version().raw())
}
