//! Text and math become glyphs here, once, when the scene file is read. The only module that uses
//! Typst, mitex and ttf-parser.
//!
//! Typst lays out both. A `text` object's string goes to Typst as a string literal, so nothing in it
//! is markup. A `math` object's `tex` goes through mitex, which rewrites LaTeX math as Typst math,
//! and then to Typst as a display equation, evaluated in the scope of mitex's command definitions.
//! Typst reads only the fonts and mitex files bundled into this binary, never the disk or the
//! network, so the same scene gives the same glyphs on every machine.
//!
//! Typst's layout is a tree of frames holding shaped glyphs and rules, such as a fraction bar.
//! Each glyph's outline and each rule becomes one `Glyph`, in the order Typst laid them out, in
//! the object's own units with y up. The object's color is Typst's text fill, so a glyph that
//! LaTeX does not color, through `\color` or `\textcolor`, has the object's color.

use std::sync::OnceLock;

use kurbo::{Affine, BezPath, Point, Shape as _, Vec2};
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::layout::{Abs, Frame, FrameItem, Transform};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font as TypstFont, FontBook, TextItem};
use typst::utils::LazyHash;
use typst::visualize::{CurveItem, Geometry, Paint, Shape as TypstShape};
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;

use crate::path::{Path, PathError};
use crate::scene::{Align, Color, Font, Glyph};

/// Libertinus Sans and Libertinus Serif for text, New Computer Modern Math for math.
static FONTS: [&[u8]; 3] = [
    include_bytes!("../assets/fonts/LibertinusSans-Regular.otf"),
    include_bytes!("../assets/fonts/LibertinusSerif-Regular.otf"),
    include_bytes!("../assets/fonts/NewCMMath-Regular.otf"),
];

/// mitex 0.2.4's Typst definitions of the LaTeX commands it rewrites. `assets/mitex/NOTICE` says
/// where they come from and how `standard.typ` is patched.
static MITEX_FILES: [(&str, &str); 3] = [
    ("/mitex/mod.typ", include_str!("../assets/mitex/mod.typ")),
    (
        "/mitex/prelude.typ",
        include_str!("../assets/mitex/prelude.typ"),
    ),
    (
        "/mitex/latex/standard.typ",
        include_str!("../assets/mitex/latex/standard.typ"),
    ),
];

/// Typst symbols that mitex 0.2.4 writes by their names before Typst 0.15, each with the name
/// Typst 0.15 gives it. A rename covers the symbol's variants too, so `sect` also renames
/// `sect.big`. mitex's converter reads its built-in command spec whatever spec it is given, so
/// the renames apply to its output.
const TYPST_0_15_RENAMES: [(&str, &str); 17] = [
    ("angle.l", "chevron.l"),
    ("angle.r", "chevron.r"),
    ("planck.reduce", "planck"),
    ("sect", "inter"),
    ("diff", "partial"),
    ("ohm.inv", "Omega.inv"),
    ("plus.circle", "plus.o"),
    ("minus.circle", "minus.o"),
    ("times.circle", "times.o"),
    ("dot.circle", "dot.o"),
    ("ast.circle", "ast.op.o"),
    ("dash.circle", "dash.o"),
    ("circle.nested", "compose.o"),
    ("arrow.l.dash", "arrow.l.dashed"),
    ("arrow.r.dash", "arrow.r.dashed"),
    ("bracket.l.double", "bracket.l.stroked"),
    ("bracket.r.double", "bracket.r.stroked"),
];

/// mitex's Typst math with each symbol renamed for Typst 0.15. Text in `[...]`, which is what
/// mitex makes of `\text{...}`, is left as written.
fn renamed_for_typst_0_15(typst_math: &str) -> String {
    let mut renamed = String::with_capacity(typst_math.len());
    let mut in_text = 0usize;
    let mut chars = typst_math.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        match c {
            '[' => in_text += 1,
            ']' => in_text = in_text.saturating_sub(1),
            _ => {}
        }
        if in_text > 0 || !c.is_ascii_alphabetic() {
            renamed.push(c);
            continue;
        }
        let mut end = start + c.len_utf8();
        while let Some(&(i, next)) = chars.peek() {
            if !(next.is_ascii_alphanumeric() || next == '.') {
                break;
            }
            end = i + next.len_utf8();
            chars.next();
        }
        let symbol = &typst_math[start..end];
        let rename = TYPST_0_15_RENAMES.iter().find_map(|&(old, new)| {
            if symbol == old {
                Some(new.to_string())
            } else {
                symbol
                    .strip_prefix(old)
                    .filter(|rest| rest.starts_with('.'))
                    .map(|rest| format!("{new}{rest}"))
            }
        });
        renamed.push_str(rename.as_deref().unwrap_or(symbol));
    }
    renamed
}

/// Typst lays out at this font size, and `size` scales the result, so a layout does not depend
/// on the size it is drawn at.
const LAYOUT_PT: f64 = 10.0;

const PAGE: &str = "#set page(width: auto, height: auto, margin: 0pt, fill: none)\n\
#set text(size: 10pt, lang: \"en\")\n\
#show math.equation: set text(font: \"New Computer Modern Math\")\n";

/// How text or math is laid out: the em size in world units, and how lines line up with each
/// other and with the object's position.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub size: f32,
    pub align: Align,
}

/// Why text or math cannot be drawn. Each message is for the agent and names no Rust type.
#[derive(Debug, PartialEq)]
pub enum TypesetError {
    /// mitex's message for LaTeX it cannot read, such as `unknown command: \foo`.
    Tex(String),
    /// Typst's message, such as `unclosed delimiter`, and for math the Typst math mitex wrote,
    /// since Typst's message points into that rather than into the LaTeX.
    Typst {
        message: String,
        typst_form: Option<String>,
    },
    /// The bundled fonts have no glyph for this character.
    MissingGlyph(char),
    /// Typst laid out nothing that draws, as for an empty or all-space string.
    NoGlyphs,
    /// A mark this tool cannot turn into a path yet, such as the stroked curve of a `\boxed`.
    Unsupported(&'static str),
    /// A color in `\color`, `\textcolor` or `\colorbox` that is neither one of `NAMED_COLORS`
    /// nor `#rrggbb`, as mitex passed it on.
    Color(String),
}

/// The color names `\color` and `\textcolor` take, in any case, from mitex's color map in
/// `assets/mitex/latex/standard.typ`. Besides these, they take `#rrggbb`.
pub const NAMED_COLORS: [&str; 17] = [
    "red",
    "green",
    "blue",
    "cyan",
    "magenta",
    "yellow",
    "black",
    "white",
    "gray",
    "lightgray",
    "darkgray",
    "brown",
    "orange",
    "pink",
    "purple",
    "teal",
    "olive",
];

/// How Typst reports the panic that the patched `get-tex-color` raises for a color it cannot
/// read. The color follows.
const UNREADABLE_COLOR: &str = "panicked with: sci_anim cannot read the color, ";

/// Glyphs with the box Typst laid them out in, `width` by `height` in the object's units, which
/// `align` placed about the object's position as it placed the glyphs: centered on it
/// vertically, and from it, about it or up to it horizontally.
pub struct Typeset {
    pub glyphs: Vec<Glyph>,
    pub width: f32,
    pub height: f32,
}

/// Plain text: `\n` breaks a line, and nothing else in it is markup.
pub fn text(
    text: &str,
    font: Font,
    color: Color,
    layout: Layout,
) -> Result<Vec<Glyph>, TypesetError> {
    text_box(text, font, color, layout).map(|set| set.glyphs)
}

/// LaTeX math, in display style, as `\[ ... \]` sets it. A glyph LaTeX does not color has `color`.
pub fn math(tex: &str, color: Color, layout: Layout) -> Result<Vec<Glyph>, TypesetError> {
    math_box(tex, color, layout).map(|set| set.glyphs)
}

/// `text`, with its box.
pub fn text_box(
    text: &str,
    font: Font,
    color: Color,
    layout: Layout,
) -> Result<Typeset, TypesetError> {
    let lines = format!("{}.split(\"\\n\").join(linebreak())", typst_string(text));
    let font = match font {
        Font::Sans => "Libertinus Sans",
        Font::Serif => "Libertinus Serif",
    };
    typeset(
        &format!(
            "#set text(font: \"{font}\")\n#set align({})\n#{lines}\n",
            typst_alignment(layout.align)
        ),
        color,
        layout,
    )
}

/// `math`, with its box.
pub fn math_box(tex: &str, color: Color, layout: Layout) -> Result<Typeset, TypesetError> {
    if let Some(bracketed) = bracketed_color(tex) {
        return Err(TypesetError::Color(bracketed.to_string()));
    }
    let typst_math = mitex::convert_math(tex, None)
        .map_err(|message| TypesetError::Tex(message.trim_start_matches("error: ").to_string()))?;
    let typst_math = renamed_for_typst_0_15(&typst_math);
    let body = format!(
        "#import \"/mitex/mod.typ\": mitex-scope\n#set align({})\n#math.equation(block: true, eval({}, mode: \"math\", scope: mitex-scope))\n",
        typst_alignment(layout.align),
        typst_string(&typst_math),
    );
    typeset(&body, color, layout).map_err(|error| match error {
        TypesetError::Typst { message, .. } => TypesetError::Typst {
            message,
            typst_form: Some(typst_math),
        },
        other => other,
    })
}

/// The color written after square brackets, such as `[rgb]{1,0,0}` in `\textcolor[rgb]{1,0,0}{x}`,
/// up to its closing brace. mitex reads no square brackets after a color command, and would set
/// the letters in them as math.
fn bracketed_color(tex: &str) -> Option<&str> {
    ["\\color", "\\textcolor", "\\colorbox"]
        .iter()
        .find_map(|command| {
            tex.match_indices(command).find_map(|(at, _)| {
                let rest = &tex[at + command.len()..];
                let bracketed = rest.trim_start();
                if rest.starts_with(|c: char| c.is_ascii_alphabetic())
                    || !bracketed.starts_with('[')
                {
                    return None;
                }
                let end = bracketed
                    .find('}')
                    .map_or(bracketed.len(), |brace| brace + 1);
                Some(&bracketed[..end])
            })
        })
}

fn typeset(body: &str, color: Color, layout: Layout) -> Result<Typeset, TypesetError> {
    let fill = format!(
        "#set text(fill: rgb({}, {}, {}))\n",
        color.r, color.g, color.b
    );
    let world = Job {
        fonts: fonts(),
        main: Source::new(file_id("/main.typ"), format!("{PAGE}{fill}{body}")),
    };
    let document = typst::compile::<PagedDocument>(&world)
        .output
        .map_err(|errors| {
            match errors
                .iter()
                .find_map(|error| error.message.strip_prefix(UNREADABLE_COLOR))
            {
                Some(color) => TypesetError::Color(color.to_string()),
                None => TypesetError::Typst {
                    message: messages(&errors),
                    typst_form: None,
                },
            }
        })?;
    let [page] = document.pages() else {
        return Err(TypesetError::Typst {
            message: "the text broke across pages".into(),
            typst_form: None,
        });
    };
    let mut glyphs = Glyphs {
        drawn: Vec::new(),
        error: None,
    };
    glyphs.frame(&page.frame, Affine::IDENTITY);
    if let Some(error) = glyphs.error {
        return Err(error);
    }
    if glyphs.drawn.is_empty() {
        return Err(TypesetError::NoGlyphs);
    }
    // Typst's frames are in points with y down. The object's units are ems of `size` with y up,
    // about the anchor `align` picks on the box Typst laid out.
    let size = page.frame.size();
    let aligned_x = match layout.align {
        Align::Left => 0.0,
        Align::Center => size.x.to_pt() / 2.0,
        Align::Right => size.x.to_pt(),
    };
    let k = f64::from(layout.size) / LAYOUT_PT;
    let to_object = Affine::new([k, 0.0, 0.0, -k, -aligned_x * k, size.y.to_pt() / 2.0 * k]);
    let mut placed = Vec::new();
    for (mut mark, color, characters) in glyphs.drawn {
        mark.apply_affine(to_object);
        match Path::from_bez(mark) {
            Ok(path) => placed.push(Glyph {
                path,
                color,
                characters,
            }),
            // An outline with no length, as some fonts give a space, draws nothing.
            Err(PathError::NoLength | PathError::Syntax(_)) => {}
            Err(PathError::TooLarge) => {
                return Err(TypesetError::Unsupported(
                    "a glyph larger than a 32-bit float holds",
                ));
            }
        }
    }
    Ok(Typeset {
        glyphs: placed,
        width: (size.x.to_pt() * k) as f32,
        height: (size.y.to_pt() * k) as f32,
    })
}

/// Every glyph outline and rule of a frame tree, in the frame's points, with its color and the
/// characters it shows.
struct Glyphs {
    drawn: Vec<(BezPath, Color, String)>,
    error: Option<TypesetError>,
}

impl Glyphs {
    fn frame(&mut self, frame: &Frame, at: Affine) {
        for (position, item) in frame.items() {
            let here = at * Affine::translate((position.x.to_pt(), position.y.to_pt()));
            match item {
                FrameItem::Group(group) => self.frame(&group.frame, here * affine(group.transform)),
                FrameItem::Text(text) => self.text(text, here),
                FrameItem::Shape(shape, _) => self.shape(shape, here),
                FrameItem::Image(..) => self.error = Some(TypesetError::Unsupported("an image")),
                FrameItem::Link(..) | FrameItem::Tag(_) => {}
            }
        }
    }

    fn text(&mut self, text: &TextItem, at: Affine) {
        let Some(color) = self.color(&text.fill) else {
            return;
        };
        let scale = text.size.to_pt() / text.font.units_per_em();
        let mut x = Abs::zero();
        for glyph in &text.glyphs {
            if glyph.id == 0 {
                let missing = text.text[glyph.range()]
                    .chars()
                    .next()
                    .unwrap_or('\u{fffd}');
                self.error
                    .get_or_insert(TypesetError::MissingGlyph(missing));
            }
            let origin = Vec2::new(
                (x + glyph.x_offset.at(text.size)).to_pt(),
                -glyph.y_offset.at(text.size).to_pt(),
            );
            // Font units are y up, and the frame is y down.
            let place = at * Affine::translate(origin) * Affine::scale_non_uniform(scale, -scale);
            let mut glyph_path = GlyphPath {
                path: BezPath::new(),
                place,
            };
            if text
                .font
                .ttf()
                .outline_glyph(ttf_parser::GlyphId(glyph.id), &mut glyph_path)
                .is_some()
            {
                self.drawn
                    .push((glyph_path.path, color, text.text[glyph.range()].to_string()));
            }
            x += glyph.x_advance.at(text.size);
        }
    }

    /// A rule, such as a fraction bar or a root's bar, is a stroked line, and becomes the filled
    /// rectangle the stroke covers, with butt ends as Typst draws it.
    fn shape(&mut self, shape: &TypstShape, at: Affine) {
        let paint = match (&shape.geometry, &shape.stroke, &shape.fill) {
            (Geometry::Line(_), Some(stroke), _) => &stroke.paint,
            (Geometry::Rect(_) | Geometry::Curve(_), None, Some(fill)) => fill,
            _ => {
                self.error.get_or_insert(TypesetError::Unsupported(
                    "a stroked rectangle or curve, such as a `\\boxed`",
                ));
                return;
            }
        };
        let Some(color) = self.color(paint) else {
            return;
        };
        let mut path = BezPath::new();
        match (&shape.geometry, &shape.stroke, &shape.fill) {
            (Geometry::Line(to), Some(stroke), _) => {
                let to = Point::new(to.x.to_pt(), to.y.to_pt());
                let along = to.to_vec2();
                let across = along.normalize().turn_90() * (stroke.thickness.to_pt() / 2.0);
                path.move_to(Point::ORIGIN + across);
                path.line_to(to + across);
                path.line_to(to - across);
                path.line_to(Point::ORIGIN - across);
                path.close_path();
            }
            (Geometry::Rect(size), None, Some(_)) => {
                path = kurbo::Rect::new(0.0, 0.0, size.x.to_pt(), size.y.to_pt()).into_path(0.0);
            }
            (Geometry::Curve(curve), None, Some(_)) => {
                let point = |p: &typst::layout::Point| Point::new(p.x.to_pt(), p.y.to_pt());
                for item in &curve.0 {
                    match item {
                        CurveItem::Move(p) => path.move_to(point(p)),
                        CurveItem::Line(p) => path.line_to(point(p)),
                        CurveItem::Cubic(a, b, p) => path.curve_to(point(a), point(b), point(p)),
                        CurveItem::Close => path.close_path(),
                    }
                }
            }
            _ => unreachable!("the paint's match takes only these three"),
        }
        path.apply_affine(at);
        self.drawn.push((path, color, String::new()));
    }

    /// A solid paint's color in sRGB. Only Typst code could make a gradient or a tiling, and
    /// neither text nor LaTeX can write one, so either is unsupported.
    fn color(&mut self, paint: &Paint) -> Option<Color> {
        let Paint::Solid(color) = paint else {
            self.error
                .get_or_insert(TypesetError::Unsupported("a gradient or tiling fill"));
            return None;
        };
        let rgb = color.to_rgb();
        let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
        Some(Color {
            r: byte(rgb.red),
            g: byte(rgb.green),
            b: byte(rgb.blue),
        })
    }
}

/// Draws a glyph's outline into a kurbo path, placed in the frame.
struct GlyphPath {
    path: BezPath,
    place: Affine,
}

impl GlyphPath {
    fn at(&self, x: f32, y: f32) -> Point {
        self.place * Point::new(f64::from(x), f64::from(y))
    }
}

impl ttf_parser::OutlineBuilder for GlyphPath {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.at(x, y);
        self.path.move_to(p);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.at(x, y);
        self.path.line_to(p);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (c, p) = (self.at(x1, y1), self.at(x, y));
        self.path.quad_to(c, p);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (c1, c2, p) = (self.at(x1, y1), self.at(x2, y2), self.at(x, y));
        self.path.curve_to(c1, c2, p);
    }

    fn close(&mut self) {
        self.path.close_path();
    }
}

fn affine(transform: Transform) -> Affine {
    Affine::new([
        transform.sx.get(),
        transform.ky.get(),
        transform.kx.get(),
        transform.sy.get(),
        transform.tx.to_pt(),
        transform.ty.to_pt(),
    ])
}

fn typst_alignment(align: Align) -> &'static str {
    match align {
        Align::Left => "left",
        Align::Center => "center",
        Align::Right => "right",
    }
}

/// A Typst string literal holding `text` exactly.
fn typst_string(text: &str) -> String {
    let mut literal = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => literal.push_str("\\\\"),
            '"' => literal.push_str("\\\""),
            '\n' => literal.push_str("\\n"),
            c if c.is_control() => literal.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => literal.push(c),
        }
    }
    literal.push('"');
    literal
}

fn messages(errors: &[SourceDiagnostic]) -> String {
    let mut message: Vec<String> = Vec::new();
    for error in errors {
        message.push(error.message.to_string());
        message.extend(error.hints.iter().map(|hint| format!("hint: {}", hint.v)));
    }
    message.join("; ")
}

/// The bundled fonts and the standard library, parsed once per process.
struct Fonts {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<TypstFont>,
}

fn fonts() -> &'static Fonts {
    static FONTS_ONCE: OnceLock<Fonts> = OnceLock::new();
    FONTS_ONCE.get_or_init(|| {
        let fonts: Vec<TypstFont> = FONTS
            .iter()
            .flat_map(|data| TypstFont::iter(Bytes::new(*data)))
            .collect();
        Fonts {
            library: LazyHash::new(Library::builder().build()),
            book: LazyHash::new(FontBook::from_fonts(&fonts)),
            fonts,
        }
    })
}

fn file_id(path: &str) -> FileId {
    FileId::new(RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new(path).expect("a bundled path is a virtual path"),
    ))
}

/// One typesetting: the bundled files plus one main source. Typst asks it for every file, so a
/// scene cannot reach the disk through `#read` or `#import`.
struct Job {
    fonts: &'static Fonts,
    main: Source,
}

impl World for Job {
    fn library(&self) -> &LazyHash<Library> {
        &self.fonts.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.fonts.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            return Ok(self.main.clone());
        }
        let path = id.vpath().get_with_slash();
        MITEX_FILES
            .iter()
            .find(|(bundled, _)| *bundled == path)
            .map(|(_, text)| Source::new(id, text.to_string()))
            .ok_or_else(|| FileError::NotFound(path.into()))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Err(FileError::NotFound(id.vpath().get_with_slash().into()))
    }

    fn font(&self, index: usize) -> Option<TypstFont> {
        self.fonts.fonts.get(index).cloned()
    }

    fn today(&self, _: Option<Duration>) -> Option<Datetime> {
        None
    }
}

/// The commands mitex knows that do not typeset as math: commands and lengths of a document
/// rather than an equation, `\limits` and `\nolimits` with no operator before them, `\Box`, whose glyph the
/// bundled math font lacks, and the boxes, whose stroked rectangle is not a glyph yet. The `tex`
/// field's description in the schema names each of them.
#[cfg(test)]
pub(crate) const DOES_NOT_TYPESET: [&str; 20] = [
    "Box",
    "bottomrule",
    "boxed",
    "cite",
    "eqref",
    "fbox",
    "footnote",
    "iffalse",
    "includegraphics",
    "item",
    "limits",
    "midrule",
    "newenvironment",
    "newenvironment*",
    "nolimits",
    "ref",
    "renewenvironment",
    "renewenvironment*",
    "textwidth",
    "toprule",
];

#[cfg(test)]
mod tests {
    use super::*;

    const CENTER: Layout = Layout {
        size: 1.0,
        align: Align::Center,
    };
    const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
    };

    fn colors(glyphs: Vec<Glyph>) -> Vec<Color> {
        glyphs.into_iter().map(|glyph| glyph.color).collect()
    }

    #[test]
    fn eulers_identity_is_seven_glyphs() {
        assert_eq!(math(r"e^{i\pi} + 1 = 0", WHITE, CENTER).unwrap().len(), 7);
    }

    #[test]
    fn a_fraction_is_its_two_glyphs_and_its_bar() {
        assert_eq!(math(r"\frac{a}{b}", WHITE, CENTER).unwrap().len(), 3);
    }

    #[test]
    fn text_is_one_path_per_glyph_after_ligatures_and_without_spaces() {
        assert_eq!(
            text("office", Font::Serif, WHITE, CENTER).unwrap().len(),
            4,
            "o, the ffi ligature, c, e"
        );
        assert_eq!(
            text("office", Font::Sans, WHITE, CENTER).unwrap().len(),
            5,
            "o, the ff ligature, i, c, e: Libertinus Sans has no ffi"
        );
        for font in [Font::Sans, Font::Serif] {
            assert_eq!(
                text("Hello, world!", font, WHITE, CENTER).unwrap().len(),
                12,
                "{font:?}"
            );
            assert_eq!(
                text("a $ * _ # [x] \\ \"", font, WHITE, CENTER)
                    .unwrap()
                    .len(),
                10,
                "nothing in text is markup in {font:?}"
            );
        }
    }

    #[test]
    fn a_glyph_latex_does_not_color_has_the_objects_color() {
        let teal = Color {
            r: 0x58,
            g: 0xc4,
            b: 0xdd,
        };
        let blue = Color { r: 0, g: 0, b: 255 };
        let red = Color { r: 255, g: 0, b: 0 };

        assert_eq!(
            colors(text("Hi", Font::Sans, teal, CENTER).unwrap()),
            [teal, teal]
        );
        assert_eq!(
            colors(math(r"x = \textcolor{blue}{y} + 1", WHITE, CENTER).unwrap()),
            [WHITE, WHITE, blue, WHITE, WHITE]
        );
        assert_eq!(
            colors(math(r"a {\color{#58C4DD} b c} d", WHITE, CENTER).unwrap()),
            [WHITE, teal, teal, WHITE],
            "`\\color` colors to the end of its group"
        );
        assert_eq!(
            colors(math(r"\textcolor{RED}{\frac{a}{b}} + \sqrt{x}", teal, CENTER).unwrap()),
            [red, red, red, teal, teal, teal, teal],
            "a fraction's bar takes its color"
        );
    }

    #[test]
    fn every_named_color_typesets_in_its_color_and_any_other_is_an_error_that_names_it() {
        for name in NAMED_COLORS {
            assert!(
                math(&format!(r"\textcolor{{{name}}}{{x}}"), WHITE, CENTER).is_ok(),
                "{name}"
            );
        }
        assert_eq!(
            colors(math(r"\textcolor{orange}{x}", WHITE, CENTER).unwrap()),
            [Color {
                r: 255,
                g: 165,
                b: 0
            }]
        );
        assert_eq!(
            math(r"\textcolor{nocolor}{x}", WHITE, CENTER).unwrap_err(),
            TypesetError::Color("nocolor".into())
        );
        assert_eq!(
            math(r"\color{#58c4d} x", WHITE, CENTER).unwrap_err(),
            TypesetError::Color("#58c4d".into())
        );
        assert_eq!(
            math(r"x \colorbox [HTML]{FF0000}{x}", WHITE, CENTER).unwrap_err(),
            TypesetError::Color("[HTML]{FF0000}".into())
        );
        assert_eq!(
            math(r"\textcolor[rgb]{1,0,0}{x}", WHITE, CENTER).unwrap_err(),
            TypesetError::Color("[rgb]{1,0,0}".into())
        );
    }

    /// mitex's Typst definitions were written for an older Typst, which named some symbols
    /// differently. Every command mitex knows must typeset with the Typst this binary links when
    /// probed as `\name{x}{y}`, except `DOES_NOT_TYPESET` and the commands whose first argument
    /// is a color or a length, which fail only because `x` is neither. Those typeset when given a
    /// real one.
    #[test]
    fn every_command_mitex_knows_typesets() {
        const WITH_A_REAL_ARGUMENT: [(&str, &str); 6] = [
            ("color", r"\color{red} x"),
            ("colorbox", r"\colorbox{red}{x}"),
            ("hspace", r"x \hspace{1em} y"),
            ("raisebox", r"\raisebox{1em}{x}"),
            ("textcolor", r"\textcolor{red}{x}"),
            ("vspace", r"x \vspace{1em} y"),
        ];
        let mut failures: Vec<&str> = mitex_spec_gen::DEFAULT_SPEC
            .items()
            .filter(|(_, item)| matches!(item, mitex::CommandSpecItem::Cmd(_)))
            .filter(|(name, _)| math(&format!("\\{name}{{x}}{{y}}"), WHITE, CENTER).is_err())
            .map(|(command, _)| command)
            .collect();
        failures.sort();
        let mut expected: Vec<&str> = DOES_NOT_TYPESET
            .iter()
            .chain(WITH_A_REAL_ARGUMENT.iter().map(|(name, _)| name))
            .copied()
            .chain([""])
            .collect();
        expected.sort();
        assert_eq!(
            failures, expected,
            "\"\" is a spec entry with no name, which no LaTeX can write"
        );

        for (_, with_a_real_argument) in WITH_A_REAL_ARGUMENT {
            assert!(
                math(with_a_real_argument, WHITE, CENTER).is_ok(),
                "{with_a_real_argument}"
            );
        }
    }

    #[test]
    fn the_schema_names_every_command_that_does_not_typeset() {
        let schema = crate::scene_file::schema().to_string();
        let tex = schema
            .split("\"description\":")
            .find(|description| description.contains("LaTeX math, such as"))
            .expect("the `tex` field's description");
        let tex =
            serde_json::from_str::<String>(tex.split_once(",\"").map_or(tex, |(d, _)| d).trim())
                .unwrap_or_default();
        let missing: Vec<&str> = DOES_NOT_TYPESET
            .iter()
            .copied()
            .filter(|name| !tex.contains(&format!("`\\{name}`")))
            .collect();
        assert_eq!(missing, Vec::<&str>::new(), "{tex}");
        assert!(tex.contains("`tabular`"), "{tex}");
    }

    /// Every environment mitex knows must typeset, probed as a 2 by 2 grid with `{cc}` for each
    /// argument, except `tabular`, which is a document's table and not math.
    #[test]
    fn every_environment_mitex_knows_typesets() {
        let grid = |name: &str, args: usize| {
            math(
                &format!(
                    "\\begin{{{name}}}{} a & b \\\\ c & d \\end{{{name}}}",
                    "{cc}".repeat(args)
                ),
                WHITE,
                CENTER,
            )
        };
        let mut failures: Vec<&str> = mitex_spec_gen::DEFAULT_SPEC
            .items()
            .filter_map(|(name, item)| match item {
                mitex::CommandSpecItem::Env(env) => Some((
                    name,
                    if let mitex::ArgPattern::FixedLenTerm { len } = env.args {
                        usize::from(len)
                    } else {
                        0
                    },
                )),
                mitex::CommandSpecItem::Cmd(_) => None,
            })
            .filter(|&(name, args)| grid(name, args).is_err())
            .map(|(name, _)| name)
            .collect();
        failures.sort();
        assert_eq!(failures, ["tabular"]);

        let glyphs = |name: &str, args: usize| grid(name, args).map(|glyphs| glyphs.len());
        assert_eq!(
            glyphs("pmatrix", 0),
            Ok(6),
            "four entries and two parentheses"
        );
        assert_eq!(glyphs("bmatrix", 0), Ok(6), "four entries and two brackets");
        assert_eq!(
            glyphs("Vmatrix", 0),
            Ok(6),
            "four entries and two double bars"
        );
        assert_eq!(glyphs("cases", 0), Ok(5), "four entries and one brace");
        assert_eq!(glyphs("aligned", 0), Ok(4));
        assert_eq!(glyphs("array", 1), Ok(4));
    }

    #[test]
    fn typst_0_15_symbols_replace_mitexs_and_text_keeps_its_words() {
        assert_eq!(
            renamed_for_typst_0_15("planck.reduce  sect.big angle.l x angle.r #textmath[sect]"),
            "planck  inter.big chevron.l x chevron.r #textmath[sect]"
        );
        assert_eq!(renamed_for_typst_0_15("sector diff"), "sector partial");
    }

    #[test]
    fn bad_tex_carries_the_message_of_mitex_or_typst() {
        assert_eq!(
            math(r"\foo{x}", WHITE, CENTER).unwrap_err(),
            TypesetError::Tex(r"unknown command: \foo".into())
        );
        assert_eq!(
            math(r"\left( x", WHITE, CENTER).unwrap_err(),
            TypesetError::Typst {
                message: "unclosed delimiter".into(),
                typst_form: Some(r"lr(\(  x ".into())
            }
        );
        assert_eq!(
            text("   ", Font::Sans, WHITE, CENTER).unwrap_err(),
            TypesetError::NoGlyphs
        );
        assert_eq!(
            text("日本", Font::Sans, WHITE, CENTER).unwrap_err(),
            TypesetError::MissingGlyph('日')
        );
    }
}
