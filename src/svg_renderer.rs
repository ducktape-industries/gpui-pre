use crate::{
    AssetSource, DevicePixels, IsZero, RenderImage, Result, SharedString, Size,
    swap_rgba_pa_to_bgra,
};
use image::{Frame, ImageFormat, ImageReader};
use resvg::tiny_skia::Pixmap;
use smallvec::SmallVec;
use std::{
    hash::Hash,
    io::{Cursor, Read},
    sync::{Arc, LazyLock, OnceLock},
};

#[cfg(target_os = "macos")]
const EMOJI_FONT_FAMILIES: &[&str] = &["Apple Color Emoji", ".AppleColorEmojiUI"];

#[cfg(target_os = "windows")]
const EMOJI_FONT_FAMILIES: &[&str] = &["Segoe UI Emoji", "Segoe UI Symbol"];

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
const EMOJI_FONT_FAMILIES: &[&str] = &[
    "Noto Color Emoji",
    "Emoji One",
    "Twitter Color Emoji",
    "JoyPixels",
];

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    target_os = "linux",
    target_os = "freebsd",
)))]
const EMOJI_FONT_FAMILIES: &[&str] = &[];

fn is_emoji_presentation(c: char) -> bool {
    static EMOJI_PRESENTATION_REGEX: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new("\\p{Emoji_Presentation}").unwrap());
    let mut buf = [0u8; 4];
    EMOJI_PRESENTATION_REGEX.is_match(c.encode_utf8(&mut buf))
}

fn font_has_char(db: &usvg::fontdb::Database, id: usvg::fontdb::ID, ch: char) -> bool {
    db.with_face_data(id, |font_data, face_index| {
        ttf_parser::Face::parse(font_data, face_index)
            .ok()
            .and_then(|face| face.glyph_index(ch))
            .is_some()
    })
    .unwrap_or(false)
}

fn select_emoji_font(
    ch: char,
    fonts: &[usvg::fontdb::ID],
    db: &usvg::fontdb::Database,
    families: &[&str],
) -> Option<usvg::fontdb::ID> {
    for family_name in families {
        let query = usvg::fontdb::Query {
            families: &[usvg::fontdb::Family::Name(family_name)],
            weight: usvg::fontdb::Weight(400),
            stretch: usvg::fontdb::Stretch::Normal,
            style: usvg::fontdb::Style::Normal,
        };

        let Some(id) = db.query(&query) else {
            continue;
        };

        if fonts.contains(&id) || !font_has_char(db, id, ch) {
            continue;
        }

        return Some(id);
    }

    None
}

/// When rendering SVGs, we render them at twice the size to get a higher-quality result.
pub const SMOOTH_SVG_SCALE_FACTOR: f32 = 2.;

#[derive(Clone, PartialEq, Hash, Eq)]
#[expect(missing_docs)]
pub struct RenderSvgParams {
    pub path: SharedString,
    pub size: Size<DevicePixels>,
}

#[derive(Clone)]
/// A struct holding everything necessary to render SVGs.
pub struct SvgRenderer {
    asset_source: Arc<dyn AssetSource>,
    usvg_options: Arc<usvg::Options<'static>>,
}

/// A parsed SVG document that can be rasterized at any scale.
///
/// Produced by [`SvgRenderer::parse_svg`] and rasterized by
/// [`SvgRenderer::render_parsed`]. Parsing resolves fonts and converts text
/// to paths, so callers that need to rasterize the same SVG at multiple
/// scales should retain this value to avoid re-paying the parse cost.
pub struct ParsedSvg(usvg::Tree);

/// The size in which to rasterize the SVG.
#[derive(Clone, Copy)]
pub enum SvgSize {
    /// A width in device pixels. The SVG retains its aspect ratio.
    Size(Size<DevicePixels>),
    /// An exact width and height in device pixels.
    ExactSize(Size<DevicePixels>),
    /// A logical scaling factor to apply to the size provided by the SVG.
    ScaleFactor(f32),
}

impl From<f32> for SvgSize {
    fn from(scale_factor: f32) -> Self {
        Self::ScaleFactor(scale_factor)
    }
}

impl SvgRenderer {
    /// Creates a new SVG renderer with the provided asset source.
    pub fn new(asset_source: Arc<dyn AssetSource>) -> Self {
        static SYSTEM_FONT_DB: LazyLock<Arc<usvg::fontdb::Database>> = LazyLock::new(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        });

        // Build the enriched font DB lazily on first SVG render rather than
        // eagerly at construction time. This avoids the expensive deep-clone
        // of the system font database for code paths that never render SVGs
        // (e.g. tests).
        let enriched_fontdb: Arc<OnceLock<Arc<usvg::fontdb::Database>>> = Arc::new(OnceLock::new());

        let default_font_resolver = usvg::FontResolver::default_font_selector();
        let font_resolver = Box::new({
            let asset_source = asset_source.clone();
            move |font: &usvg::Font, db: &mut Arc<usvg::fontdb::Database>| {
                if db.is_empty() {
                    let fontdb = enriched_fontdb.get_or_init(|| {
                        let mut db = (**SYSTEM_FONT_DB).clone();
                        load_bundled_fonts(&*asset_source, &mut db);
                        fix_generic_font_families(&mut db);
                        Arc::new(db)
                    });
                    *db = fontdb.clone();
                }
                if let Some(id) = default_font_resolver(font, db) {
                    return Some(id);
                }
                // fontdb doesn't recognize CSS system font keywords like "system-ui"
                // or "ui-sans-serif", so fall back to sans-serif before any face.
                let sans_query = usvg::fontdb::Query {
                    families: &[usvg::fontdb::Family::SansSerif],
                    ..Default::default()
                };
                db.query(&sans_query)
                    .or_else(|| db.faces().next().map(|f| f.id))
            }
        });
        let default_fallback_selection = usvg::FontResolver::default_fallback_selector();
        let fallback_selection = Box::new(
            move |ch: char, fonts: &[usvg::fontdb::ID], db: &mut Arc<usvg::fontdb::Database>| {
                if is_emoji_presentation(ch) {
                    if let Some(id) = select_emoji_font(ch, fonts, db.as_ref(), EMOJI_FONT_FAMILIES)
                    {
                        return Some(id);
                    }
                }

                default_fallback_selection(ch, fonts, db)
            },
        );
        let options = usvg::Options {
            font_resolver: usvg::FontResolver {
                select_font: font_resolver,
                select_fallback: fallback_selection,
            },
            image_href_resolver: usvg::ImageHrefResolver {
                resolve_data: Box::new(raster_data_href),
                // usvg's default reads any file an href names (`/dev/zero`, a
                // FIFO, the person's own pictures). An href is never read.
                resolve_string: Box::new(|_, _| None),
            },
            ..Default::default()
        };
        Self {
            asset_source,
            usvg_options: Arc::new(options),
        }
    }

    /// Parses SVG data into a [`ParsedSvg`] that can be rasterized at any scale.
    pub fn parse_svg(&self, bytes: &[u8]) -> Result<ParsedSvg, usvg::Error> {
        parse_tree(bytes, &self.usvg_options).map(ParsedSvg)
    }

    /// Rasterizes a previously parsed SVG into an image buffer.
    pub fn render_parsed(
        &self,
        svg: &ParsedSvg,
        size: impl Into<SvgSize>,
    ) -> Result<Arc<RenderImage>, usvg::Error> {
        let (size, image_scale_factor) = match size.into() {
            SvgSize::Size(size) => (SvgSize::Size(size), 1.0),
            SvgSize::ExactSize(size) => (SvgSize::ExactSize(size), 1.0),
            SvgSize::ScaleFactor(scale_factor) => (
                SvgSize::ScaleFactor(scale_factor * SMOOTH_SVG_SCALE_FACTOR),
                SMOOTH_SVG_SCALE_FACTOR,
            ),
        };
        let pixmap = rasterize_tree(&svg.0, size)?;
        let mut buffer =
            image::ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take()).unwrap();

        for pixel in buffer.chunks_exact_mut(4) {
            swap_rgba_pa_to_bgra(pixel);
        }

        let mut image = RenderImage::new(SmallVec::from_const([Frame::new(buffer)]));
        image.scale_factor = image_scale_factor;
        Ok(Arc::new(image))
    }

    /// Renders the given bytes into an image buffer.
    pub fn render_single_frame(
        &self,
        bytes: &[u8],
        scale_factor: f32,
    ) -> Result<Arc<RenderImage>, usvg::Error> {
        let svg = self.parse_svg(bytes)?;
        self.render_parsed(&svg, scale_factor)
    }

    pub(crate) fn render_alpha_mask(
        &self,
        params: &RenderSvgParams,
        bytes: Option<&[u8]>,
    ) -> Result<Option<(Size<DevicePixels>, Vec<u8>)>> {
        anyhow::ensure!(!params.size.is_zero(), "can't render at a zero size");

        let render_pixmap = |bytes| {
            let pixmap = self.render_pixmap(bytes, SvgSize::Size(params.size))?;

            // Convert the pixmap's pixels into an alpha mask.
            let size = Size::new(
                DevicePixels(pixmap.width() as i32),
                DevicePixels(pixmap.height() as i32),
            );
            let alpha_mask = pixmap
                .pixels()
                .iter()
                .map(|p| p.alpha())
                .collect::<Vec<_>>();

            Ok(Some((size, alpha_mask)))
        };

        if let Some(bytes) = bytes {
            render_pixmap(bytes)
        } else if let Some(bytes) = self.asset_source.load(&params.path)? {
            render_pixmap(&bytes)
        } else {
            Ok(None)
        }
    }

    fn render_pixmap(&self, bytes: &[u8], size: SvgSize) -> Result<Pixmap, usvg::Error> {
        let tree = parse_tree(bytes, &self.usvg_options)?;
        rasterize_tree(&tree, size)
    }
}

/// How far a gzip-compressed (svgz) document may inflate. A plain SVG cannot
/// amplify itself; a compressed one can, and usvg inflates it without a bound.
/// 16 MiB is sixteen times the largest picture a ducktape view may send.
const MAX_SVGZ_INFLATED_BYTES: u64 = 16 << 20;

fn parse_tree(bytes: &[u8], options: &usvg::Options) -> Result<usvg::Tree, usvg::Error> {
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return usvg::Tree::from_data(bytes, options);
    }
    let mut inflated = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .take(MAX_SVGZ_INFLATED_BYTES + 1)
        .read_to_end(&mut inflated)
        .map_err(|_| usvg::Error::MalformedGZip)?;
    if inflated.len() as u64 > MAX_SVGZ_INFLATED_BYTES {
        log::warn!("svgz inflates past {MAX_SVGZ_INFLATED_BYTES} bytes; refused");
        return Err(usvg::Error::MalformedGZip);
    }
    // `from_str`, not `from_data`: gzip inside gzip would inflate unbounded.
    let text = std::str::from_utf8(&inflated).map_err(|_| usvg::Error::NotAnUtf8Str)?;
    usvg::Tree::from_str(text, options)
}

/// The widest or tallest raster an SVG data URL may declare: the rasterizer's
/// own cap (`rasterize_tree`'s `MAX_SIZE`).
const MAX_RASTER_SIDE: u32 = 8192;
/// The most pixels an SVG data URL raster may declare: 64 MiB of RGBA.
const MAX_RASTER_PIXELS: u64 = 1 << 24;

fn raster_fits(width: u32, height: u32) -> bool {
    width <= MAX_RASTER_SIDE
        && height <= MAX_RASTER_SIDE
        && u64::from(width) * u64::from(height) <= MAX_RASTER_PIXELS
}

/// Whether every `VP8 ` and `VP8L` frame of a WebP, at the top level and
/// inside each `ANMF` frame, fits at its own declared size. image-webp decodes
/// a frame whole before it compares the frame with the `VP8X` canvas, so the
/// canvas size alone does not bound the decode. The walk runs to the end of
/// the bytes, not to the declared RIFF size: image-webp reads chunks past it.
/// A chunk that does not read refuses the image.
fn webp_frames_fit(data: &[u8]) -> bool {
    data.get(8..12) == Some(&b"WEBP"[..])
        && data
            .get(12..)
            .is_some_and(|chunks| webp_chunks_fit(chunks, true))
}

fn webp_chunks_fit(mut chunks: &[u8], top_level: bool) -> bool {
    let side = |low: u8, high: u8| u32::from(u16::from_le_bytes([low, high]) & 0x3FFF);
    while let Some(header) = chunks.get(..8) {
        let size = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
        let rest = &chunks[8..];
        let Some(body) = rest.get(..size) else {
            return false;
        };
        let fits = match &header[..4] {
            // The frame header's 14-bit width and height, after the start code.
            b"VP8 " => body
                .get(6..10)
                .is_some_and(|d| raster_fits(side(d[0], d[1]), side(d[2], d[3]))),
            // 14-bit width - 1 and height - 1, after the 0x2f signature.
            b"VP8L" => body.get(1..5).is_some_and(|d| {
                let bits = u32::from_le_bytes(d.try_into().unwrap());
                raster_fits((bits & 0x3FFF) + 1, (bits >> 14 & 0x3FFF) + 1)
            }),
            // A 16-byte frame header, then the frame's own chunks. Frames do
            // not nest, so neither does this walk.
            b"ANMF" => top_level && body.get(16..).is_some_and(|f| webp_chunks_fit(f, false)),
            _ => true,
        };
        if !fits {
            return false;
        }
        chunks = rest.get(size + size % 2..).unwrap_or_default();
    }
    true
}

/// Whether a GIF's first frame fits: resvg decodes that frame at its own size,
/// whatever the logical screen declares.
fn gif_first_frame_fits(data: &[u8]) -> bool {
    let Ok(mut decoder) = gif::DecodeOptions::new().read_info(data) else {
        return false;
    };
    matches!(decoder.next_frame_info(), Ok(Some(frame))
        if raster_fits(frame.width.into(), frame.height.into()))
}

/// An `<image>` or `<feImage>` data URL resolves to a raster image only, as
/// usvg's default does for rasters. A nested SVG (`image/svg+xml`, or
/// `text/plain` without raster magic) is refused: usvg would parse it, and
/// inflate it without a bound when it is gzip. A raster whose header declares
/// more than [`MAX_RASTER_SIDE`] a side or [`MAX_RASTER_PIXELS`] is refused
/// too, and so is a WebP or GIF frame that does: resvg decodes them whole, and
/// a small WebP or JPEG can declare 1 GiB.
fn raster_data_href(mime: &str, data: Arc<Vec<u8>>, _: &usvg::Options) -> Option<usvg::ImageKind> {
    let format = match mime {
        "image/jpg" | "image/jpeg" => ImageFormat::Jpeg,
        "image/png" => ImageFormat::Png,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::WebP,
        "text/plain" => image::guess_format(&data).ok()?,
        _ => return None,
    };
    let kind: fn(Arc<Vec<u8>>) -> usvg::ImageKind = match format {
        ImageFormat::Jpeg => usvg::ImageKind::JPEG,
        ImageFormat::Png => usvg::ImageKind::PNG,
        ImageFormat::Gif => usvg::ImageKind::GIF,
        ImageFormat::WebP => usvg::ImageKind::WEBP,
        _ => return None,
    };
    // Headers only: no pixel is decoded. `no_limits` so these caps are the rule.
    let mut reader = ImageReader::with_format(Cursor::new(data.as_slice()), format);
    reader.no_limits();
    let (width, height) = reader.into_dimensions().ok()?;
    let frames_fit = match format {
        ImageFormat::WebP => webp_frames_fit(&data),
        ImageFormat::Gif => gif_first_frame_fits(&data),
        _ => true,
    };
    if !raster_fits(width, height) || !frames_fit {
        log::warn!(
            "SVG raster of {width}x{height}, or a frame in it, is too large or unreadable; refused"
        );
        return None;
    }
    Some(kind(data))
}

fn rasterize_tree(tree: &usvg::Tree, size: SvgSize) -> Result<Pixmap, usvg::Error> {
    // Cap the size of the rendered pixmap to avoid texture allocation panics
    // Related issue: #56466
    const MAX_SIZE: f32 = 8192.0;

    let svg_size = tree.size();
    let (mut width, mut height) = match size {
        SvgSize::Size(size) => {
            let scale = i32::from(size.width) as f32 / svg_size.width();
            (svg_size.width() * scale, svg_size.height() * scale)
        }
        SvgSize::ExactSize(size) => (i32::from(size.width) as f32, i32::from(size.height) as f32),
        SvgSize::ScaleFactor(scale) => (svg_size.width() * scale, svg_size.height() * scale),
    };

    if width > MAX_SIZE {
        log::warn!("Attempted to render pixmap where width ({width}) > MAX_SIZE ({MAX_SIZE})");
    }
    if height > MAX_SIZE {
        log::warn!("Attempted to render pixmap where height ({height}) > MAX_SIZE ({MAX_SIZE})");
    }
    let scale = (MAX_SIZE / width).min(MAX_SIZE / height).min(1.0);
    width *= scale;
    height *= scale;

    // Render the SVG to a pixmap with the specified width and height.
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width as u32, height as u32)
        .ok_or(usvg::Error::InvalidSize)?;

    let transform = resvg::tiny_skia::Transform::from_scale(
        width / svg_size.width(),
        height / svg_size.height(),
    );

    resvg::render(tree, transform, &mut pixmap.as_mut());

    Ok(pixmap)
}

fn load_bundled_fonts(asset_source: &dyn AssetSource, db: &mut usvg::fontdb::Database) {
    let font_paths = [
        "fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf",
        "fonts/lilex/Lilex-Regular.ttf",
    ];
    for path in font_paths {
        match asset_source.load(path) {
            Ok(Some(data)) => db.load_font_data(data.into_owned()),
            Ok(None) => log::warn!("Bundled font not found: {path}"),
            Err(error) => log::warn!("Failed to load bundled font {path}: {error}"),
        }
    }
}

// fontdb defaults generic families to Microsoft fonts ("Arial", "Times New Roman")
// which aren't installed on most Linux systems. fontconfig normally overrides these,
// but when it fails the defaults remain and all generic family queries return None.
fn fix_generic_font_families(db: &mut usvg::fontdb::Database) {
    use usvg::fontdb::{Family, Query};

    let families_and_fallbacks: &[(Family<'_>, &str)] = &[
        (Family::SansSerif, "IBM Plex Sans"),
        // No serif font bundled; use sans-serif as best available fallback.
        (Family::Serif, "IBM Plex Sans"),
        (Family::Monospace, "Lilex"),
        (Family::Cursive, "IBM Plex Sans"),
        (Family::Fantasy, "IBM Plex Sans"),
    ];

    for (family, fallback_name) in families_and_fallbacks {
        let query = Query {
            families: &[*family],
            ..Default::default()
        };
        if db.query(&query).is_none() {
            match family {
                Family::SansSerif => db.set_sans_serif_family(*fallback_name),
                Family::Serif => db.set_serif_family(*fallback_name),
                Family::Monospace => db.set_monospace_family(*fallback_name),
                Family::Cursive => db.set_cursive_family(*fallback_name),
                Family::Fantasy => db.set_fantasy_family(*fallback_name),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usvg::fontdb::{Database, Family, Query};

    const IBM_PLEX_REGULAR: &[u8] =
        include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf");
    const LILEX_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/lilex/Lilex-Regular.ttf");

    #[test]
    fn renders_parsed_svg_at_requested_size() -> Result<()> {
        let renderer = SvgRenderer::new(Arc::new(()));
        let svg = renderer.parse_svg(
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="24pt" height="12pt"></svg>"#,
        )?;
        let requested_size = Size::new(DevicePixels(24), DevicePixels(12));
        let image = renderer.render_parsed(&svg, SvgSize::ExactSize(requested_size))?;

        assert_eq!(image.size(0), requested_size);
        Ok(())
    }

    #[test]
    fn preserves_aspect_ratio_for_width_constrained_size() -> Result<()> {
        let renderer = SvgRenderer::new(Arc::new(()));
        let svg = renderer.parse_svg(
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="24pt" height="12pt"></svg>"#,
        )?;
        let image = renderer.render_parsed(
            &svg,
            SvgSize::Size(Size::new(DevicePixels(24), DevicePixels(24))),
        )?;

        assert_eq!(image.size(0), Size::new(DevicePixels(24), DevicePixels(12)));
        Ok(())
    }

    /// `<svg width="4" height="4"><rect width="4" height="4"/></svg>`, base64.
    const SUB_SVG: &str = "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSI0IiBoZWlnaHQ9IjQiPjxyZWN0IHdpZHRoPSI0IiBoZWlnaHQ9IjQiLz48L3N2Zz4=";
    /// [`SUB_SVG`] gzip-compressed, base64.
    const SUB_SVGZ: &str = "H4sIAAAAAAACA7MpLktXqMjNySu2VcooKSmw0tcvLy/XKzfWyy9K1zcyMDDQB6pQUijPTCnJsFUyUVLISM1MzygBMe1silKTS7BK6dvZgPTZAQAT6jFqXwAAAA==";
    /// A 1x1 opaque red PNG, base64.
    const RED_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

    fn find_image(group: &usvg::Group) -> Option<&usvg::Image> {
        group.children().iter().find_map(|node| match node {
            usvg::Node::Image(image) => Some(&**image),
            usvg::Node::Group(group) => find_image(group),
            _ => None,
        })
    }

    fn svg_with_image(href: &str) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><image href="{href}" width="4" height="4"/></svg>"#
        )
    }

    #[test]
    fn svg_image_href_never_reads_a_local_file() -> Result<()> {
        let path = std::env::temp_dir().join(format!("gpui-svg-href-{}.png", std::process::id()));
        image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255])).save(&path)?;
        let renderer = SvgRenderer::new(Arc::new(()));
        let svg = renderer.parse_svg(svg_with_image(&path.to_string_lossy()).as_bytes());
        std::fs::remove_file(&path)?;

        assert!(find_image(svg?.0.root()).is_none());
        Ok(())
    }

    #[test]
    fn svg_nested_svg_data_href_is_refused() -> Result<()> {
        let renderer = SvgRenderer::new(Arc::new(()));
        let mut shown = Vec::new();
        for href in [
            format!("data:image/svg+xml;base64,{SUB_SVG}"),
            format!("data:text/plain;base64,{SUB_SVG}"),
            format!("data:image/svg+xml;base64,{SUB_SVGZ}"),
        ] {
            let svg = renderer.parse_svg(svg_with_image(&href).as_bytes())?;
            if find_image(svg.0.root()).is_some() {
                shown.push(href);
            }
        }
        assert_eq!(shown, Vec::<String>::new());
        Ok(())
    }

    #[test]
    fn svg_raster_data_href_still_renders() -> Result<()> {
        let renderer = SvgRenderer::new(Arc::new(()));
        for href in [
            format!("data:image/png;base64,{RED_PNG}"),
            format!("data:;base64,{RED_PNG}"),
        ] {
            let svg = renderer.parse_svg(svg_with_image(&href).as_bytes())?;
            let image = find_image(svg.0.root());
            assert!(
                matches!(image.map(|i| i.kind()), Some(usvg::ImageKind::PNG(_))),
                "{href}"
            );
            let size = Size::new(DevicePixels(4), DevicePixels(4));
            let pixmap =
                renderer.render_pixmap(svg_with_image(&href).as_bytes(), SvgSize::Size(size))?;
            assert_eq!(pixmap.pixel(2, 2).map(|p| p.red()), Some(255), "{href}");
        }
        Ok(())
    }

    /// `bytes` as a percent-encoded data URL.
    fn data_url(mime: &str, bytes: &[u8]) -> String {
        let body: String = bytes.iter().map(|byte| format!("%{byte:02X}")).collect();
        format!("data:{mime},{body}")
    }

    /// Whether an `<image>` with this href parses to an image node.
    fn shown(renderer: &SvgRenderer, href: &str) -> Result<bool> {
        let svg = renderer.parse_svg(svg_with_image(href).as_bytes())?;
        Ok(find_image(svg.0.root()).is_some())
    }

    /// A PNG of header chunks only (no pixel data) declaring `width` x
    /// `height`.
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let chunk = |kind: &[u8], body: &[u8]| {
            let mut crc = flate2::Crc::new();
            crc.update(kind);
            crc.update(body);
            let len = (body.len() as u32).to_be_bytes();
            [&len[..], kind, body, &crc.sum().to_be_bytes()].concat()
        };
        let ihdr = [
            &width.to_be_bytes()[..],
            &height.to_be_bytes(),
            &[8, 6, 0, 0, 0],
        ]
        .concat();
        [
            &b"\x89PNG\r\n\x1a\n"[..],
            &chunk(b"IHDR", &ihdr),
            &chunk(b"IDAT", &[]),
            &chunk(b"IEND", &[]),
        ]
        .concat()
    }

    #[test]
    fn svg_raster_data_href_past_the_size_caps_is_refused() -> Result<()> {
        let renderer = SvgRenderer::new(Arc::new(()));
        let png = |(width, height)| data_url("image/png", &png_header(width, height));
        // At both caps: 8192 a side and 8192 * 2048 = MAX_RASTER_PIXELS.
        assert!(shown(&renderer, &png((MAX_RASTER_SIDE, 2048)))?);

        let mut over = Vec::new();
        for size in [(16383, 16383), (MAX_RASTER_SIDE + 1, 1), (4097, 4097)] {
            if shown(&renderer, &png(size))? {
                over.push(size);
            }
        }
        assert_eq!(over, Vec::<(u32, u32)>::new());
        Ok(())
    }

    fn riff_chunk(fourcc: &[u8], body: &[u8]) -> Vec<u8> {
        let pad: &[u8] = if body.len() % 2 == 1 { &[0] } else { &[] };
        [fourcc, &(body.len() as u32).to_le_bytes(), body, pad].concat()
    }

    /// A `VP8 ` key frame header (no pixel data) declaring `width` x `height`.
    fn vp8(width: u16, height: u16) -> Vec<u8> {
        let header = [0, 0, 0, 0x9d, 0x01, 0x2a];
        let body = [&header[..], &width.to_le_bytes(), &height.to_le_bytes()].concat();
        riff_chunk(b"VP8 ", &body)
    }

    /// A `VP8L` header (no pixel data) declaring `width` x `height`.
    fn vp8l(width: u32, height: u32) -> Vec<u8> {
        let bits = (width - 1) | (height - 1) << 14;
        riff_chunk(b"VP8L", &[&[0x2f][..], &bits.to_le_bytes()].concat())
    }

    /// An extended WebP: a `VP8X` canvas of `canvas` x `canvas` around
    /// `frame`, which an `ANIM` + `ANMF` pair wraps when `animated`.
    fn webp(canvas: u32, animated: bool, frame: Vec<u8>) -> String {
        webp_file(canvas, animated, frame, true)
    }

    /// [`webp`] whose RIFF size ends before the frame (`VP8 `/`VP8L`, or the
    /// `ANMF`): image-webp reads chunks past the declared end anyway.
    fn webp_frame_past_riff_end(canvas: u32, animated: bool, frame: Vec<u8>) -> String {
        webp_file(canvas, animated, frame, false)
    }

    fn webp_file(canvas: u32, animated: bool, frame: Vec<u8>, riff_covers_frame: bool) -> String {
        let less_one = (canvas - 1).to_le_bytes();
        let flags = if animated { 0x02 } else { 0 };
        let vp8x = [&[flags, 0, 0, 0][..], &less_one[..3], &less_one[..3]].concat();
        let mut chunks = riff_chunk(b"VP8X", &vp8x);
        if animated {
            chunks.extend(riff_chunk(b"ANIM", &[0; 6]));
        }
        let before_frame = chunks.len();
        if animated {
            let anmf = [&[0; 6][..], &less_one[..3], &less_one[..3], &[0; 4], &frame].concat();
            chunks.extend(riff_chunk(b"ANMF", &anmf));
        } else {
            chunks.extend(frame);
        }
        let covered = if riff_covers_frame {
            chunks.len()
        } else {
            before_frame
        };
        let riff_size = (4 + covered as u32).to_le_bytes();
        let file = [&b"RIFF"[..], &riff_size, b"WEBP", &chunks].concat();
        data_url("image/webp", &file)
    }

    #[test]
    fn svg_webp_frame_past_the_caps_is_refused_whatever_the_canvas() -> Result<()> {
        let renderer = SvgRenderer::new(Arc::new(()));
        // A real encoded WebP, and the same shapes at an honest small size,
        // read and show.
        let mut encoded = Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]))
            .write_to(&mut encoded, ImageFormat::WebP)?;
        for href in [
            data_url("image/webp", encoded.get_ref()),
            webp(4, false, vp8(4, 4)),
            webp(4, false, vp8l(4, 4)),
            webp(4, true, vp8(4, 4)),
            webp_frame_past_riff_end(4, false, vp8(4, 4)),
            webp_frame_past_riff_end(4, true, vp8(4, 4)),
        ] {
            assert!(shown(&renderer, &href)?, "{href}");
        }

        let mut shown_over = Vec::new();
        for (name, href) in [
            ("VP8X 1x1 around VP8 8200", webp(1, false, vp8(8200, 8200))),
            (
                "VP8X 1x1 around VP8L 8200",
                webp(1, false, vp8l(8200, 8200)),
            ),
            ("ANMF frame VP8 8200", webp(1, true, vp8(8200, 8200))),
            (
                "VP8 8200 past the RIFF end",
                webp_frame_past_riff_end(1, false, vp8(8200, 8200)),
            ),
            (
                "ANMF frame VP8 8200 past the RIFF end",
                webp_frame_past_riff_end(1, true, vp8(8200, 8200)),
            ),
        ] {
            if shown(&renderer, &href)? {
                shown_over.push(name);
            }
        }
        assert_eq!(shown_over, Vec::<&str>::new());
        Ok(())
    }

    /// A GIF header (no pixel data): a `screen` x `screen` logical screen
    /// with a two-color table, and a first frame of `width` x `height`.
    fn gif_header(screen: u16, width: u16, height: u16) -> Vec<u8> {
        [
            &b"GIF89a"[..],
            &screen.to_le_bytes(),
            &screen.to_le_bytes(),
            &[0x80, 0, 0, 0, 0, 0, 255, 255, 255, 0x2c, 0, 0, 0, 0],
            &width.to_le_bytes(),
            &height.to_le_bytes(),
            &[0, 2, 0, 0x3b],
        ]
        .concat()
    }

    #[test]
    fn svg_gif_first_frame_past_the_caps_is_refused_whatever_the_screen() -> Result<()> {
        let renderer = SvgRenderer::new(Arc::new(()));
        let gif = |screen, width, height| data_url("image/gif", &gif_header(screen, width, height));
        assert!(shown(&renderer, &gif(4, 4, 4))?);
        assert!(!shown(&renderer, &gif(1, 65535, 190))?);
        Ok(())
    }

    #[test]
    fn svgz_inflating_past_the_cap_is_refused() -> Result<()> {
        use std::io::Write;
        let gzip = |svg: &[u8]| -> std::io::Result<Vec<u8>> {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder.write_all(svg)?;
            encoder.finish()
        };
        let renderer = SvgRenderer::new(Arc::new(()));
        let open = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">"#;
        let small = gzip(&[&open[..], b"</svg>"].concat())?;
        assert!(renderer.parse_svg(&small).is_ok());

        let padding = vec![b' '; MAX_SVGZ_INFLATED_BYTES as usize];
        let bomb = gzip(&[&open[..], &padding, b"</svg>"].concat())?;
        assert!(matches!(
            renderer.parse_svg(&bomb),
            Err(usvg::Error::MalformedGZip)
        ));
        Ok(())
    }

    fn db_with_bundled_fonts() -> Database {
        let mut db = Database::new();
        db.load_font_data(IBM_PLEX_REGULAR.to_vec());
        db.load_font_data(LILEX_REGULAR.to_vec());
        db
    }

    #[test]
    fn text_with_split_glyph_clusters_in_mixed_fonts_does_not_panic() {
        let mut db = Database::new();
        db.load_font_data(IBM_PLEX_REGULAR.to_vec());
        db.load_font_data(LILEX_REGULAR.to_vec());
        let options = usvg::Options {
            fontdb: std::sync::Arc::new(db),
            ..Default::default()
        };

        // A base letter followed by a stack of combining marks. Under HarfBuzz's
        // default cluster merging every mark glyph shares the base's byte index,
        // which is the "glyph splitting" condition that triggered the panic. The
        // chunk must use two different fonts so the buggy merge path runs.
        let zalgo = "e\u{0301}\u{0302}\u{0303}\u{0304}\u{0306}\u{0307}\u{0308}\u{030a}";
        let svg = format!(
            r#"<svg viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg"><text font-family="Lilex" font-size="32">{zalgo}<tspan font-family="IBM Plex Sans">{zalgo}</tspan></text></svg>"#
        );

        // Before the fix this aborts via panic with a message like
        // "removal index (is 5) should be < len (is 5)".
        usvg::Tree::from_data(svg.as_bytes(), &options)
            .expect("SVG with mixed-font text should parse");
    }

    #[test]
    fn test_is_emoji_presentation() {
        let cases = [
            ("a", false),
            ("Z", false),
            ("1", false),
            ("#", false),
            ("*", false),
            ("漢", false),
            ("中", false),
            ("カ", false),
            ("©", false),
            ("♥", false),
            ("😀", true),
            ("✅", true),
            ("🇺🇸", true),
            // SVG fallback is not cluster-aware yet
            ("©️", false),
            ("♥️", false),
            ("1️⃣", false),
        ];
        for (s, expected) in cases {
            assert_eq!(
                is_emoji_presentation(s.chars().next().unwrap()),
                expected,
                "for char {:?}",
                s
            );
        }
    }

    #[test]
    fn fix_generic_font_families_sets_all_families() {
        let mut db = db_with_bundled_fonts();
        fix_generic_font_families(&mut db);

        let families = [
            Family::SansSerif,
            Family::Serif,
            Family::Monospace,
            Family::Cursive,
            Family::Fantasy,
        ];

        for family in families {
            let query = Query {
                families: &[family],
                ..Default::default()
            };
            assert!(
                db.query(&query).is_some(),
                "Expected generic family {family:?} to resolve after fix_generic_font_families"
            );
        }
    }

    #[test]
    fn test_select_emoji_font_skips_family_without_glyph() {
        let mut db = db_with_bundled_fonts();

        let ibm_plex_sans = db
            .query(&usvg::fontdb::Query {
                families: &[usvg::fontdb::Family::Name("IBM Plex Sans")],
                weight: usvg::fontdb::Weight(400),
                stretch: usvg::fontdb::Stretch::Normal,
                style: usvg::fontdb::Style::Normal,
            })
            .unwrap();
        let lilex = db
            .query(&usvg::fontdb::Query {
                families: &[usvg::fontdb::Family::Name("Lilex")],
                weight: usvg::fontdb::Weight(400),
                stretch: usvg::fontdb::Stretch::Normal,
                style: usvg::fontdb::Style::Normal,
            })
            .unwrap();
        let selected = select_emoji_font('│', &[], &db, &["IBM Plex Sans", "Lilex"]).unwrap();

        assert_eq!(selected, lilex);
        assert!(!font_has_char(&db, ibm_plex_sans, '│'));
        assert!(font_has_char(&db, selected, '│'));
    }

    #[test]
    fn fix_generic_font_families_monospace_resolves_to_lilex() {
        let mut db = db_with_bundled_fonts();
        fix_generic_font_families(&mut db);

        let query = Query {
            families: &[Family::Monospace],
            ..Default::default()
        };
        let id = db.query(&query).expect("Monospace should resolve");
        let face = db.face(id).expect("Face should exist");
        assert!(
            face.families.iter().any(|(name, _)| name.contains("Lilex")),
            "Monospace should map to Lilex, got {:?}",
            face.families
        );
    }
}
