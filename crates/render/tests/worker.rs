//! The worker process under its limits: a post that never ends, or eats
//! memory, is stopped, and the supervisor says so.

use motet_render::job::{Failure, Job, Limits};
use motet_render::supervisor::Supervisor;

fn supervisor() -> Supervisor {
    Supervisor::new(env!("CARGO_BIN_EXE_motet-render-worker"), None)
}

fn job(source: &str) -> Job {
    Job {
        source: source.into(),
        date: (2026, 10, 19),
        limits: Limits::default(),
    }
}

#[tokio::test]
async fn a_post_renders_through_the_worker() {
    let outcome = supervisor()
        .render(&job("Hello, *World*."))
        .await
        .expect("worker starts");
    assert_eq!(outcome.map(|r| r.pages.len()), Ok(1));
}

#[tokio::test]
async fn endless_loops_are_stopped() {
    let mut endless = job("#for i in range(100000) { for j in range(100000) { } }");
    endless.limits.max_cpu_seconds = 1;
    endless.limits.max_wall_millis = 3_000;
    let outcome = supervisor().render(&endless).await.expect("worker starts");
    assert_eq!(outcome, Err(Failure::Timeout));
}

#[tokio::test]
async fn memory_hogs_are_stopped() {
    let mut hog = job("#let s = \"x\"\n#for i in range(40) { s = s + s }\n#s.len()");
    hog.limits.max_memory_bytes = 256 * 1024 * 1024;
    let outcome = supervisor().render(&hog).await.expect("worker starts");
    // Whichever limit it meets first (memory or CPU), it never renders.
    assert!(
        matches!(outcome, Err(Failure::Crashed | Failure::Timeout)),
        "{outcome:?}"
    );
}
