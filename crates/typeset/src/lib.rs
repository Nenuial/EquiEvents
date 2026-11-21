//! The show office's PDFs: a Typst template (`templates/*.typ`) filled with the print's data (JSON,
//! read by the template as `/data.json`), laid out and written as PDF by Typst, all in memory.
//!
//! The data is built by the office (`office::prints`); the templates only lay it out, so what a
//! print says is tested on its data and on the text of its pages ([`text`]), not on its looks.
//!
//! Fonts: the system's sans-serif (Helvetica Neue on macOS; Segoe UI or Arial on Windows; DejaVu
//! or Liberation on Linux), with Typst's own fonts as the fallback, so a print never fails for
//! want of a font.

use std::path::Path;
use std::sync::OnceLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::layout::{Frame, FrameItem, PagedDocument};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};

/// A kind of print.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    StartList,
    Ranking,
    JudgeSheets,
    Combined,
    Participants,
    Schedule,
    Labels,
    Letters,
    StartCards,
    StyleSheets,
    Officials,
    Entries,
    TeamCards,
    ControlSheets,
}

impl Template {
    fn source(self) -> &'static str {
        match self {
            Template::StartList => include_str!("../templates/startlist.typ"),
            Template::Ranking => include_str!("../templates/ranking.typ"),
            Template::JudgeSheets => include_str!("../templates/judgesheets.typ"),
            Template::Combined => include_str!("../templates/combined.typ"),
            Template::Participants => include_str!("../templates/participants.typ"),
            Template::Schedule => include_str!("../templates/schedule.typ"),
            Template::Labels => include_str!("../templates/labels.typ"),
            Template::Letters => include_str!("../templates/letters.typ"),
            Template::StartCards => include_str!("../templates/startcards.typ"),
            Template::StyleSheets => include_str!("../templates/stylesheets.typ"),
            Template::Officials => include_str!("../templates/officials.typ"),
            Template::Entries => include_str!("../templates/entries.typ"),
            Template::TeamCards => include_str!("../templates/teamcards.typ"),
            Template::ControlSheets => include_str!("../templates/controlsheets.typ"),
        }
    }
}

/// What every template imports: page, header, footer, tables.
const COMMON: &str = include_str!("../templates/common.typ");

/// Pictures the templates use: the federation's logos on its official forms (the dressage
/// judges' sheets), as Rosson prints them.
const ASSETS: [(&str, &[u8]); 2] = [("assets/swiss-equestrian.png", include_bytes!("../assets/swiss-equestrian.png")), ("assets/fei.png", include_bytes!("../assets/fei.png"))];

/// A finished print.
pub struct Pdf {
    pub bytes: Vec<u8>,
    pub pages: usize,
}

/// Lay out a print and write it as PDF.
pub fn render(template: Template, data: &serde_json::Value) -> Result<Pdf, String> {
    let document = layout(template, data)?;
    let bytes = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|errors| messages(&errors))?;
    Ok(Pdf { bytes, pages: document.pages.len() })
}

/// The text of each page, in reading order as laid out (for tests: what a print says).
pub fn text(template: Template, data: &serde_json::Value) -> Result<Vec<String>, String> {
    let document = layout(template, data)?;
    Ok(document
        .pages
        .iter()
        .map(|page| {
            let mut out = String::new();
            collect(&page.frame, &mut out);
            out
        })
        .collect())
}

fn collect(frame: &Frame, out: &mut String) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Group(group) => collect(&group.frame, out),
            FrameItem::Text(text) => {
                out.push_str(text.text.as_str());
                out.push(' ');
            }
            _ => {}
        }
    }
}

fn layout(template: Template, data: &serde_json::Value) -> Result<PagedDocument, String> {
    let world = PrintWorld::new(template, data)?;
    let result = typst::compile::<PagedDocument>(&world);
    result.output.map_err(|errors| {
        // Where in the template (file and line), to find a template's mistake.
        errors
            .iter()
            .map(|e| {
                let at = e.span.id().and_then(|id| typst::World::source(&world, id).ok()).and_then(|source| {
                    let range = source.range(e.span)?;
                    Some(format!("{}:{}: ", source.id().vpath().as_rootless_path().display(), source.byte_to_line(range.start)? + 1))
                });
                format!("{}{}", at.unwrap_or_default(), e.message)
            })
            .collect::<Vec<_>>()
            .join("; ")
    })
}

fn messages(errors: &[typst::diag::SourceDiagnostic]) -> String {
    errors.iter().map(|e| e.message.to_string()).collect::<Vec<_>>().join("; ")
}

/// The fonts, found once.
struct Fonts {
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

fn fonts() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut fonts: Vec<Font> = Vec::new();
        for (dir, prefixes) in system_font_files() {
            let Ok(entries) = std::fs::read_dir(dir) else { continue };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if !prefixes.iter().any(|p| name.starts_with(p)) {
                    continue;
                }
                if let Ok(bytes) = std::fs::read(entry.path()) {
                    fonts.extend(Font::iter(Bytes::new(bytes)));
                }
            }
        }
        fonts.extend(typst_assets::fonts().flat_map(|data| Font::iter(Bytes::new(data))));
        Fonts { book: LazyHash::new(FontBook::from_fonts(&fonts)), fonts }
    })
}

/// Where the system's sans-serif fonts are, and which files (by the start of their name).
fn system_font_files() -> Vec<(&'static Path, &'static [&'static str])> {
    if cfg!(target_os = "macos") {
        vec![(Path::new("/System/Library/Fonts"), &["helveticaneue"]), (Path::new("/System/Library/Fonts/Supplemental"), &["arial"])]
    } else if cfg!(target_os = "windows") {
        vec![(Path::new("C:\\Windows\\Fonts"), &["segoeui", "arial"])]
    } else {
        vec![
            (Path::new("/usr/share/fonts/truetype/dejavu"), &["dejavusans"]),
            (Path::new("/usr/share/fonts/truetype/liberation"), &["liberationsans"]),
            (Path::new("/usr/share/fonts/TTF"), &["dejavusans", "liberationsans"]),
        ]
    }
}

/// Typst's view of a print: the template, the shared parts, the data, its images.
struct PrintWorld {
    library: LazyHash<Library>,
    main: Source,
    common: Source,
    data: Bytes,
    /// Images given in the data as data URLs (a class's logo), by their file name.
    images: Vec<(String, Bytes)>,
}

impl PrintWorld {
    fn new(template: Template, data: &serde_json::Value) -> Result<PrintWorld, String> {
        let id = |path: &str| FileId::new(None, VirtualPath::new(path));
        let mut data = data.clone();
        let mut images = Vec::new();
        take_images(&mut data, &mut images);
        let data = serde_json::to_vec(&data).map_err(|e| e.to_string())?;
        Ok(PrintWorld {
            library: LazyHash::new(Library::default()),
            main: Source::new(id("/main.typ"), template.source().to_string()),
            common: Source::new(id("/common.typ"), COMMON.to_string()),
            data: Bytes::new(data),
            images,
        })
    }
}

/// Every data URL of a PNG or JPEG image in the data becomes a file (`/images/3.png`) and the
/// string its path, for the template's `image(…)`; one that is not an image becomes `null`.
fn take_images(value: &mut serde_json::Value, images: &mut Vec<(String, Bytes)>) {
    match value {
        serde_json::Value::String(text) if text.starts_with("data:image") => {
            let bytes = text.split_once(";base64,").and_then(|(_, b64)| decode_base64(b64));
            let kind = match bytes.as_deref() {
                Some([0x89, b'P', b'N', b'G', ..]) => Some("png"),
                Some([0xFF, 0xD8, ..]) => Some("jpg"),
                _ => None,
            };
            *value = match (bytes, kind) {
                (Some(bytes), Some(kind)) => {
                    let name = format!("/images/{}.{kind}", images.len());
                    images.push((name.clone(), Bytes::new(bytes)));
                    serde_json::Value::String(name)
                }
                _ => serde_json::Value::Null,
            };
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(|v| take_images(v, images)),
        serde_json::Value::Object(map) => map.values_mut().for_each(|v| take_images(v, images)),
        _ => {}
    }
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    };
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        buffer = (buffer << 6) | u32::from(value(c)?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

impl World for PrintWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &fonts().book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            Ok(self.main.clone())
        } else if id == self.common.id() {
            Ok(self.common.clone())
        } else {
            Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = id.vpath().as_rootless_path();
        if path == Path::new("data.json") {
            return Ok(self.data.clone());
        }
        if let Some((_, bytes)) = ASSETS.iter().find(|(name, _)| Path::new(name) == path) {
            return Ok(Bytes::new(bytes.to_vec()));
        }
        match self.images.iter().find(|(name, _)| Path::new(name.trim_start_matches('/')) == path) {
            Some((_, bytes)) => Ok(bytes.clone()),
            None => Err(FileError::NotFound(path.into())),
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        fonts().fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        // Prints say when they were made from their data, not from the clock.
        None
    }
}
