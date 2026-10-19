//! What a post can and can't reach, compiled in-process.

use motet_render::job::{Failure, Job, Limits};
use motet_render::worker::render;

fn job(source: &str) -> Job {
    Job {
        source: source.into(),
        date: (2026, 10, 19),
        limits: Limits::default(),
    }
}

#[test]
fn a_post_renders_to_svg() {
    let rendered = render(&job("= The Gilded Reach\nThe gate *creaks* open."), None).expect("renders");
    assert_eq!(rendered.pages.len(), 1);
    assert!(rendered.pages[0].starts_with("<svg"));
}

#[test]
fn rendering_is_deterministic() {
    let source = "#lorem(40)\n\n#datetime.today().display()";
    assert_eq!(render(&job(source), None), render(&job(source), None));
}

#[test]
fn the_date_is_the_posts_own() {
    let a = render(&job("#datetime.today().display()"), None);
    let mut other = job("#datetime.today().display()");
    other.date = (2027, 1, 1);
    assert_ne!(a, render(&other, None));
}

#[test]
fn no_file_outside_the_post_can_be_read() {
    for source in [
        r#"#read("/etc/passwd")"#,
        r#"#read("../../../../etc/passwd")"#,
        r#"#image("/etc/hostname")"#,
        r#"#include "other.typ""#,
    ] {
        assert!(
            matches!(render(&job(source), None), Err(Failure::Source(_))),
            "{source}"
        );
    }
}

#[test]
fn packages_are_refused_without_a_mirror() {
    let outcome = render(&job(r#"#import "@preview/cmarker:0.1.6": render"#), None);
    assert!(matches!(outcome, Err(Failure::Source(_))), "{outcome:?}");
}

#[test]
fn oversized_sources_and_outputs_are_refused() {
    let mut long = job("a");
    long.limits.max_source_bytes = 0;
    assert_eq!(render(&long, None), Err(Failure::TooLong(0)));

    let mut large = job("#lorem(200)");
    large.limits.max_output_bytes = 100;
    assert_eq!(render(&large, None), Err(Failure::TooLarge(100)));
}

#[test]
fn compile_errors_are_reported_and_deterministic() {
    let outcome = render(&job("#let x = "), None);
    let Err(failure) = outcome else { panic!("should fail") };
    assert!(failure.is_deterministic());
}
