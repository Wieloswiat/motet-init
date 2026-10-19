//! The only things a post can reach while it compiles.

use std::path::PathBuf;
use std::sync::LazyLock;

use typst::diag::{FileError, FileResult, PackageError};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

struct Fonts {
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

/// The bundled fonts only: no system fonts, so output doesn't depend on the
/// machine and no font file outside the binary is ever parsed.
static FONTS: LazyLock<Fonts> = LazyLock::new(|| {
    let fonts: Vec<Font> = typst_assets::fonts()
        .flat_map(|data| Font::iter(Bytes::new(data)))
        .collect();
    Fonts {
        book: LazyHash::new(FontBook::from_fonts(&fonts)),
        fonts,
    }
});

static LIBRARY: LazyLock<LazyHash<Library>> = LazyLock::new(|| LazyHash::new(Library::default()));

/// One post's compilation environment.
pub struct Sandbox {
    main: Source,
    date: Option<Datetime>,
    /// Read-only package mirror: `{namespace}/{name}/{version}/…`, filled
    /// outside the worker with names, sizes and hashes checked. `None`
    /// means no packages at all.
    packages: Option<PathBuf>,
}

impl std::fmt::Debug for Sandbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sandbox")
            .field("packages", &self.packages)
            .finish_non_exhaustive()
    }
}

impl Sandbox {
    pub fn new(source: String, date: (i32, u8, u8), packages: Option<PathBuf>) -> Self {
        let main = RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::new("/post.typ").expect("a valid virtual path"),
        )
        .intern();
        Self {
            main: Source::new(main, source),
            date: Datetime::from_ymd(date.0, date.1, date.2),
            packages,
        }
    }

    fn package_file(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.get();
        let VirtualRoot::Package(spec) = path.root() else {
            // Nothing in the project but the post itself.
            return Err(FileError::AccessDenied);
        };
        let mirror = self
            .packages
            .as_ref()
            .ok_or_else(|| FileError::Package(PackageError::NotFound(spec.clone())))?;
        let dir = mirror
            .join(spec.namespace.as_str())
            .join(spec.name.as_str())
            .join(spec.version.to_string());
        // `realize` refuses paths that would escape the package directory.
        let file = path.vpath().realize(&dir).map_err(FileError::Realize)?;
        std::fs::read(&file)
            .map(Bytes::new)
            .map_err(|e| FileError::from_io(e, &file))
    }
}

impl World for Sandbox {
    fn library(&self) -> &LazyHash<Library> {
        &LIBRARY
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &FONTS.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            return Ok(self.main.clone());
        }
        let bytes = self.package_file(id)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| FileError::InvalidUtf8)?;
        Ok(Source::new(id, text.to_owned()))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main.id() {
            return Ok(Bytes::from_string(self.main.clone()));
        }
        self.package_file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        FONTS.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        self.date
    }
}
