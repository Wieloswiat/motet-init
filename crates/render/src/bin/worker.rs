//! `motet-render-worker [package-mirror]`: renders one job from stdin.
//! Started by `motet_render::supervisor`, never by hand.

fn main() -> std::io::Result<()> {
    let packages = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    motet_render::worker::run(packages)
}
