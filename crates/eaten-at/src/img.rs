//! The cover-art proxy: one stable image URL per document.
//!
//! Every reader's browser and every unfurler hits this endpoint rather
//! than the author's PDS. Behind it sits a fallback chain (blob, then
//! a generated placeholder) whose result can change
//! without the URL changing. Input is treated as hostile: only raster
//! formats are accepted, dimensions are checked before decoding, and the
//! output is always a freshly encoded JPEG.

use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader, Rgb, RgbImage};
use url::Url;

use crate::cache::Namespace;
use crate::error::AppError;
use crate::model::SubjectDocument;
use crate::state::AppState;

/// Largest source image we will download, in bytes. The lexicon caps
/// blobs at 1 MB; allow room for a PDS that does not enforce it.
pub const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
/// Largest cover blob `site.standard.document` allows, in bytes.
pub const MAX_COVER_BYTES: usize = 1_000_000;
/// Long side a cover is shrunk to when it must be re-encoded to fit.
const COVER_FIT_DIMENSION: u32 = 1600;
/// Largest source dimensions we will decode.
const MAX_SOURCE_DIMENSION: u32 = 6000;
/// Decode memory budget handed to the image crate.
const MAX_DECODE_BYTES: u64 = 96 * 1024 * 1024;
/// JPEG quality of everything we emit.
const JPEG_QUALITY: u8 = 84;

/// Which rendition of a document's cover is wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Size {
    /// Square-ish, at most 800px on the long side. Subject cards and listings.
    #[default]
    Card,
    /// 1200×630 canvas for OpenGraph unfurls: the cover centred on a
    /// background taken from its own average color.
    Og,
}

impl Size {
    pub fn from_query(value: Option<&str>) -> Self {
        match value {
            Some("og") => Self::Og,
            _ => Self::Card,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Card => "card",
            Self::Og => "og",
        }
    }
}

/// Where a cover came from. Reported in a response header for debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Blob,
    Placeholder,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blob => "blob",
            Self::Placeholder => "placeholder",
        }
    }
}

/// A ready-to-serve JPEG.
#[derive(Debug, Clone)]
pub struct Rendition {
    pub jpeg: Vec<u8>,
}

impl AppState {
    /// The cover for `subject_doc` at `size`, cached. Never fails: the last
    /// link in the chain is a generated placeholder.
    pub async fn cover_rendition(
        &self,
        identity: &eaten_at_atproto::identity::Identity,
        subject_doc: &SubjectDocument,
        size: Size,
    ) -> Rendition {
        let key = format!("{}/{}/{}", identity.did, subject_doc.rkey(), size.as_str());
        let cached = self
            .cache()
            .get_or_fetch_bytes::<AppError, _, _>(Namespace::Image, &key, || async {
                Ok(Some(self.resolve_cover(identity, subject_doc, size).await))
            })
            .await;
        match cached {
            Ok(Some(jpeg)) => Rendition { jpeg },
            _ => Rendition {
                jpeg: self.resolve_cover(identity, subject_doc, size).await,
            },
        }
    }

    /// A publication's icon at OG size: the icon blob if it decodes,
    /// else a placeholder seeded by the publication name. Cached.
    pub async fn icon_rendition(
        &self,
        identity: &eaten_at_atproto::identity::Identity,
        publication: &eaten_at_atproto::repo::Record<eaten_at_atproto::lexicon::Publication>,
    ) -> Rendition {
        let key = format!("{}/{}/icon", identity.did, publication.rkey());
        let build = || async {
            let image = match &publication.value.icon {
                Some(blob) => {
                    let url = self.repo_for(identity).blob_url(&identity.did, blob.cid());
                    self.fetch_and_decode(url).await
                }
                None => None,
            }
            .unwrap_or_else(|| placeholder_from_seed(&publication.value.name));
            tokio::task::spawn_blocking(move || encode(&image, Size::Og))
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_default()
        };
        let cached = self
            .cache()
            .get_or_fetch_bytes::<AppError, _, _>(Namespace::Image, &key, || async {
                Ok(Some(build().await))
            })
            .await;
        match cached {
            Ok(Some(jpeg)) => Rendition { jpeg },
            _ => Rendition {
                jpeg: build().await,
            },
        }
    }

    /// Walk the fallback chain and encode the first usable source.
    async fn resolve_cover(
        &self,
        identity: &eaten_at_atproto::identity::Identity,
        subject_doc: &SubjectDocument,
        size: Size,
    ) -> Vec<u8> {
        let (source, image) = self.source_image(identity, subject_doc).await;
        tracing::debug!(did = %identity.did, rkey = subject_doc.rkey(), source = source.as_str(), "cover resolved");
        match tokio::task::spawn_blocking(move || encode(&image, size)).await {
            Ok(Ok(jpeg)) => jpeg,
            Ok(Err(err)) => {
                tracing::warn!(%err, "cover encode failed; using placeholder");
                encode(&placeholder(subject_doc), size).unwrap_or_default()
            }
            Err(err) => {
                tracing::warn!(%err, "cover encode task failed; using placeholder");
                encode(&placeholder(subject_doc), size).unwrap_or_default()
            }
        }
    }

    async fn source_image(
        &self,
        identity: &eaten_at_atproto::identity::Identity,
        subject_doc: &SubjectDocument,
    ) -> (Source, DynamicImage) {
        if let Some(blob) = &subject_doc.document().cover_image {
            let url = self.repo_for(identity).blob_url(&identity.did, blob.cid());
            if let Some(image) = self.fetch_and_decode(url).await {
                return (Source::Blob, image);
            }
        }
        (Source::Placeholder, placeholder(subject_doc))
    }

    /// Download and decode an image, or `None` with a log line for any
    /// reason at all: this is a fallback chain, not an error path.
    async fn fetch_and_decode(&self, url: Url) -> Option<DynamicImage> {
        let response = match self.http().get_limited(url.clone(), MAX_SOURCE_BYTES).await {
            Ok(r) => r,
            Err(err) => {
                tracing::debug!(%url, %err, "cover fetch failed");
                return None;
            }
        };
        if !response.status.is_success() {
            tracing::debug!(%url, status = %response.status, "cover fetch not successful");
            return None;
        }
        let content_type = response.content_type().unwrap_or("").to_ascii_lowercase();
        if !is_raster_content_type(&content_type) {
            tracing::debug!(%url, content_type, "cover is not a raster image");
            return None;
        }
        let bytes = response.body;
        match tokio::task::spawn_blocking(move || decode(&bytes)).await {
            Ok(Ok(image)) => Some(image),
            Ok(Err(err)) => {
                tracing::debug!(%url, %err, "cover decode rejected");
                None
            }
            Err(err) => {
                tracing::warn!(%url, %err, "cover decode task failed");
                None
            }
        }
    }
}

/// Only formats we can decode and that cannot carry scripts. SVG is the
/// sharp edge and is deliberately absent.
fn is_raster_content_type(content_type: &str) -> bool {
    let mime = content_type.split(';').next().unwrap_or("").trim();
    matches!(
        mime,
        "image/jpeg" | "image/png" | "image/gif" | "image/webp"
    )
}

/// Why an image was rejected or could not be produced.
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("unsupported or unrecognised image format")]
    Format,
    #[error("image dimensions {0}×{1} exceed the limit")]
    TooLarge(u32, u32),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("read error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image could not be brought under {0} bytes")]
    CannotShrink(usize),
}

/// Prepare an author's upload for storage as a cover blob: a raster
/// image, under the lexicon's size cap. A small enough JPEG, PNG, GIF, or
/// WebP is kept as uploaded; a larger one is shrunk and re-encoded as
/// JPEG. Returns the bytes and their MIME type.
pub fn cover_upload(bytes: &[u8]) -> Result<(Vec<u8>, &'static str), ImageError> {
    let format = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .format()
        .ok_or(ImageError::Format)?;
    let image = decode(bytes)?;
    if bytes.len() <= MAX_COVER_BYTES {
        let mime = match format {
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::Gif => "image/gif",
            ImageFormat::WebP => "image/webp",
            _ => return Err(ImageError::Format),
        };
        return Ok((bytes.to_vec(), mime));
    }
    // Smaller and coarser until it fits. A photograph fits on the first
    // try; only something like pure noise gets as far as the last.
    for limit in [COVER_FIT_DIMENSION, 1200, 800] {
        let (w, h) = image.dimensions();
        let fitted = if w.max(h) > limit {
            image.resize(limit, limit, FilterType::Lanczos3)
        } else {
            image.clone()
        };
        let rgb = fitted.to_rgb8();
        for quality in [JPEG_QUALITY, 72, 60] {
            let mut out = Cursor::new(Vec::new());
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
            rgb.write_with_encoder(encoder)?;
            let out = out.into_inner();
            if out.len() <= MAX_COVER_BYTES {
                return Ok((out, "image/jpeg"));
            }
        }
    }
    Err(ImageError::CannotShrink(MAX_COVER_BYTES))
}

/// Decode with format sniffed from the bytes (never from the declared
/// type), dimensions checked before pixels are allocated.
pub fn decode(bytes: &[u8]) -> Result<DynamicImage, ImageError> {
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let format = reader.format().ok_or(ImageError::Format)?;
    if !matches!(
        format,
        ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::Gif | ImageFormat::WebP
    ) {
        return Err(ImageError::Format);
    }
    let mut reader = reader;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_DIMENSION);
    limits.max_image_height = Some(MAX_SOURCE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let (w, h) = reader.into_dimensions()?;
    if w > MAX_SOURCE_DIMENSION || h > MAX_SOURCE_DIMENSION || w == 0 || h == 0 {
        return Err(ImageError::TooLarge(w, h));
    }
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_DIMENSION);
    limits.max_image_height = Some(MAX_SOURCE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    Ok(reader.decode()?)
}

/// Fit the image to `size` and encode it as JPEG.
pub fn encode(image: &DynamicImage, size: Size) -> Result<Vec<u8>, ImageError> {
    let framed = match size {
        Size::Card => {
            let (w, h) = image.dimensions();
            if w.max(h) > 800 {
                image.resize(800, 800, FilterType::Lanczos3)
            } else {
                image.clone()
            }
        }
        Size::Og => frame_for_og(image),
    };
    let mut out = Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
    framed.to_rgb8().write_with_encoder(encoder)?;
    Ok(out.into_inner())
}

/// Centre the image on a 1200×630 canvas filled with its average color.
fn frame_for_og(image: &DynamicImage) -> DynamicImage {
    const W: u32 = 1200;
    const H: u32 = 630;
    let fitted = image.resize(W, H, FilterType::Lanczos3);
    let mut canvas = RgbImage::from_pixel(W, H, average_color(image));
    let (fw, fh) = fitted.dimensions();
    let x = i64::from((W - fw) / 2);
    let y = i64::from((H - fh) / 2);
    image::imageops::overlay(&mut canvas, &fitted.to_rgb8(), x, y);
    DynamicImage::ImageRgb8(canvas)
}

fn average_color(image: &DynamicImage) -> Rgb<u8> {
    let small = image.resize_exact(8, 8, FilterType::Triangle).to_rgb8();
    let mut sum = [0u64; 3];
    for px in small.pixels() {
        for (i, c) in px.0.iter().enumerate() {
            sum[i] += u64::from(*c);
        }
    }
    let n = u64::from(small.width() * small.height()).max(1);
    // Averages of u8 values are within u8 range by construction.
    #[allow(clippy::cast_possible_truncation)]
    Rgb([(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8])
}

/// A deterministic placeholder: two-tone blocks derived from the subject's
/// title, so the same subject always gets the same image and different
/// subjects are told apart at a glance.
pub fn placeholder(subject_doc: &SubjectDocument) -> DynamicImage {
    placeholder_from_seed(&subject_doc.subject.title)
}

/// [`placeholder`] for an arbitrary seed string.
pub fn placeholder_from_seed(seed: &str) -> DynamicImage {
    const SIZE: u32 = 600;
    const CELLS: u32 = 5;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in seed.bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    let hue = f64::from(u32::try_from(hash % 360).unwrap_or(0));
    let base = hsl_to_rgb(hue, 0.35, 0.28);
    let tone = hsl_to_rgb((hue + 30.0) % 360.0, 0.55, 0.55);
    let mut img = RgbImage::from_pixel(SIZE, SIZE, base);
    let cell = SIZE / CELLS;
    let mut bits = hash >> 9;
    // Mirror horizontally so the pattern reads as intentional.
    for row in 0..CELLS {
        for col in 0..CELLS.div_ceil(2) {
            let on = bits & 1 == 1;
            bits >>= 1;
            if !on {
                continue;
            }
            for c in [col, CELLS - 1 - col] {
                for y in row * cell..(row + 1) * cell {
                    for x in c * cell..(c + 1) * cell {
                        img.put_pixel(x, y, tone);
                    }
                }
            }
        }
    }
    DynamicImage::ImageRgb8(img)
}

fn hsl_to_rgb(hue: f64, saturation: f64, lightness: f64) -> Rgb<u8> {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let secondary = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let offset = lightness - chroma / 2.0;
    // Hue is already reduced modulo 360, so the cast is exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let sector = hue as u32 / 60;
    let (red, green, blue) = match sector {
        0 => (chroma, secondary, 0.0),
        1 => (secondary, chroma, 0.0),
        2 => (0.0, chroma, secondary),
        3 => (0.0, secondary, chroma),
        4 => (secondary, 0.0, chroma),
        _ => (chroma, 0.0, secondary),
    };
    // Values are in 0..=1 before scaling, so the casts cannot truncate or lose sign.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Rgb([
        ((red + offset) * 255.0).round() as u8,
        ((green + offset) * 255.0).round() as u8,
        ((blue + offset) * 255.0).round() as u8,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(w, h, |x, y| {
            let channel = |v: u32| u8::try_from(v % 256).unwrap_or(0);
            Rgb([channel(x), channel(y), 128])
        });
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn cover_uploads_are_kept_or_shrunk_to_the_blob_cap() {
        let small = png(64, 64);
        let (bytes, mime) = cover_upload(&small).unwrap();
        assert_eq!(mime, "image/png");
        assert_eq!(bytes, small, "a small file is kept as uploaded");

        // Noise does not compress: this PNG is several megabytes.
        let mut seed = 0x9e37_79b9_u32;
        let noisy = RgbImage::from_fn(1800, 1800, |_, _| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let b = seed.to_be_bytes();
            Rgb([b[0], b[1], b[2]])
        });
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(noisy)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        let big = out.into_inner();
        assert!(big.len() > MAX_COVER_BYTES, "{}", big.len());
        let (bytes, mime) = cover_upload(&big).unwrap();
        assert_eq!(mime, "image/jpeg");
        assert!(bytes.len() <= MAX_COVER_BYTES, "{}", bytes.len());
        let shrunk = decode(&bytes).unwrap();
        assert!(shrunk.dimensions().0.max(shrunk.dimensions().1) <= COVER_FIT_DIMENSION);

        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#;
        assert!(matches!(cover_upload(svg), Err(ImageError::Format)));
    }

    #[test]
    fn decodes_png_and_rejects_svg_and_garbage() {
        assert!(decode(&png(10, 10)).is_ok());
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
        assert!(matches!(decode(svg), Err(ImageError::Format)));
        assert!(matches!(decode(b"not an image"), Err(ImageError::Format)));
        assert!(!is_raster_content_type("image/svg+xml"));
        assert!(is_raster_content_type("image/png; charset=binary"));
        assert!(!is_raster_content_type("text/html"));
    }

    #[test]
    fn oversized_dimensions_are_rejected_before_decode() {
        // A PNG header claiming 7000×7000 with no real pixel data.
        let mut bytes = png(1, 1);
        // IHDR width/height live at bytes 16..24 (big-endian u32s).
        bytes[16..20].copy_from_slice(&7000u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&7000u32.to_be_bytes());
        assert!(matches!(
            decode(&bytes),
            Err(ImageError::TooLarge(7000, 7000) | ImageError::Image(_))
        ));
    }

    #[test]
    fn card_encoding_fits_within_800_and_is_jpeg() {
        let big = decode(&png(1600, 900)).unwrap();
        let jpeg = encode(&big, Size::Card).unwrap();
        assert_eq!(&jpeg[..2], &[0xff, 0xd8], "JPEG magic");
        let back = decode(&jpeg).unwrap();
        assert_eq!(back.dimensions(), (800, 450));
        let small = decode(&png(300, 300)).unwrap();
        let back = decode(&encode(&small, Size::Card).unwrap()).unwrap();
        assert_eq!(
            back.dimensions(),
            (300, 300),
            "small images are not upscaled"
        );
    }

    #[test]
    fn og_encoding_is_always_1200_by_630() {
        for (w, h) in [(300, 300), (2000, 500), (500, 2000)] {
            let img = decode(&png(w, h)).unwrap();
            let back = decode(&encode(&img, Size::Og).unwrap()).unwrap();
            assert_eq!(back.dimensions(), (1200, 630), "{w}x{h}");
        }
    }

    #[test]
    fn placeholder_is_deterministic_and_distinct() {
        let a = encode(&placeholder_from_seed("Sample Subject"), Size::Card).unwrap();
        let a2 = encode(&placeholder_from_seed("Sample Subject"), Size::Card).unwrap();
        let b = encode(&placeholder_from_seed("Second Subject"), Size::Card).unwrap();
        assert_eq!(a, a2);
        assert_ne!(a, b);
        assert_eq!(decode(&a).unwrap().dimensions(), (600, 600));
    }
}
