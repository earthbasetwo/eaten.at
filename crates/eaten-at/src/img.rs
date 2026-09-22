//! The image proxy: one stable URL per document image, and one per
//! photo.
//!
//! Every reader's browser and every unfurler hits this endpoint rather
//! than the author's PDS. Behind the document image sits a fallback
//! chain (the first photo, a cover another client set, then a generated
//! placeholder) whose result can change without the URL changing. Input
//! is treated as hostile: only raster formats are accepted, dimensions
//! are checked before decoding, and the output is always a freshly
//! encoded JPEG. Uploads go the same way (plan 07): re-encoded, so no
//! metadata block, and with it no camera position, reaches the repo.

use std::io::Cursor;

use eaten_at_atproto::lexicon::MAX_PHOTO_BYTES;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{
    DynamicImage, GenericImageView, ImageDecoder, ImageFormat, ImageReader, Rgb, RgbImage,
};
use url::Url;

use crate::cache::Namespace;
use crate::error::AppError;
use crate::model::VisitDocument;
use crate::state::AppState;

/// Largest source image we will download, in bytes. The lexicon caps
/// blobs at 1 MB; allow room for a PDS that does not enforce it.
pub const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
/// Largest image blob we write, in bytes: the lexicon caps a photo, and
/// Standard caps a cover, at the same size.
pub const MAX_IMAGE_BLOB_BYTES: usize = MAX_PHOTO_BYTES;
/// Largest photo file accepted for upload, before it is re-encoded.
pub const MAX_PHOTO_UPLOAD_BYTES: usize = 10 * 1024 * 1024;
/// Long side a photo is shrunk to on upload.
const PHOTO_FIT_DIMENSION: u32 = 2048;
/// Side of a square photo thumbnail.
const THUMB_SIDE: u32 = 400;
/// Long side of a photo's full rendition.
const FULL_FIT_DIMENSION: u32 = 1600;
/// Widest a listing card's photo is served (plan 13), cropped to 3:2.
const CARD_WIDTH: u32 = 960;
/// Largest source dimensions we will decode.
const MAX_SOURCE_DIMENSION: u32 = 6000;
/// Decode memory budget handed to the image crate.
const MAX_DECODE_BYTES: u64 = 96 * 1024 * 1024;
/// JPEG quality of everything we emit.
const JPEG_QUALITY: u8 = 84;

/// Which rendition of a document's cover is wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Size {
    /// Square-ish, at most 800px on the long side. Visit cards and listings.
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

/// Which rendition of a photo is wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PhotoSize {
    /// A 400px square, centre-cropped: the photo grid and the photos page.
    #[default]
    Thumb,
    /// Fitted to 1600px on the long side, its own shape.
    Full,
    /// Up to 960px wide, centre-cropped to 3:2, never upscaled: the
    /// listing card (plan 13).
    Card,
}

impl PhotoSize {
    pub fn from_query(value: Option<&str>) -> Self {
        match value {
            Some("full") => Self::Full,
            Some("card") => Self::Card,
            _ => Self::Thumb,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Thumb => "thumb",
            Self::Full => "full",
            Self::Card => "card",
        }
    }
}

/// Where a document's image came from. Reported in the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Photo,
    Blob,
    Placeholder,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Photo => "photo",
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
    /// The cover for `visit_doc` at `size`, cached. Never fails: the last
    /// link in the chain is a generated placeholder.
    pub async fn cover_rendition(
        &self,
        identity: &eaten_at_atproto::identity::Identity,
        visit_doc: &VisitDocument,
        size: Size,
    ) -> Rendition {
        let key = format!("{}/{}/{}", identity.did, visit_doc.rkey(), size.as_str());
        let cached = self
            .cache()
            .get_or_fetch_bytes::<AppError, _, _>(Namespace::Image, &key, || async {
                Ok(Some(self.resolve_cover(identity, visit_doc, size).await))
            })
            .await;
        match cached {
            Ok(Some(jpeg)) => Rendition { jpeg },
            _ => Rendition {
                jpeg: self.resolve_cover(identity, visit_doc, size).await,
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
        visit_doc: &VisitDocument,
        size: Size,
    ) -> Vec<u8> {
        let (source, image) = self.source_image(identity, visit_doc).await;
        tracing::debug!(did = %identity.did, rkey = visit_doc.rkey(), source = source.as_str(), "cover resolved");
        match tokio::task::spawn_blocking(move || encode(&image, size)).await {
            Ok(Ok(jpeg)) => jpeg,
            Ok(Err(err)) => {
                tracing::warn!(%err, "cover encode failed; using placeholder");
                encode(&placeholder(visit_doc), size).unwrap_or_default()
            }
            Err(err) => {
                tracing::warn!(%err, "cover encode task failed; using placeholder");
                encode(&placeholder(visit_doc), size).unwrap_or_default()
            }
        }
    }

    async fn source_image(
        &self,
        identity: &eaten_at_atproto::identity::Identity,
        visit_doc: &VisitDocument,
    ) -> (Source, DynamicImage) {
        if let Some(photo) = visit_doc.visit.photos.first() {
            let url = self
                .repo_for(identity)
                .blob_url(&identity.did, photo.image.cid());
            if let Some(image) = self.fetch_and_decode(url).await {
                return (Source::Photo, image);
            }
        }
        if let Some(blob) = &visit_doc.document().cover_image {
            let url = self.repo_for(identity).blob_url(&identity.did, blob.cid());
            if let Some(image) = self.fetch_and_decode(url).await {
                return (Source::Blob, image);
            }
        }
        (Source::Placeholder, placeholder(visit_doc))
    }

    /// One of the document's photos at `size`, cached; `None` when the
    /// document lists no photo with that CID (this is not an open blob
    /// proxy) or the blob cannot be read.
    pub async fn photo_rendition(
        &self,
        identity: &eaten_at_atproto::identity::Identity,
        visit_doc: &VisitDocument,
        cid: &str,
        size: PhotoSize,
    ) -> Option<Rendition> {
        let photo = visit_doc
            .visit
            .photos
            .iter()
            .find(|p| p.image.cid() == cid)?;
        let key = format!(
            "{}/{}/{}/{}",
            identity.did,
            visit_doc.rkey(),
            photo.image.cid(),
            size.as_str()
        );
        let build = || async {
            let url = self
                .repo_for(identity)
                .blob_url(&identity.did, photo.image.cid());
            let image = self.fetch_and_decode(url).await?;
            tokio::task::spawn_blocking(move || encode_photo(&image, size))
                .await
                .ok()
                .and_then(Result::ok)
        };
        let cached = self
            .cache()
            .get_or_fetch_bytes::<AppError, _, _>(Namespace::Image, &key, || async {
                Ok(build().await)
            })
            .await;
        match cached {
            Ok(Some(jpeg)) => Some(Rendition { jpeg }),
            Ok(None) => None,
            Err(_) => build().await.map(|jpeg| Rendition { jpeg }),
        }
    }

    /// Keep private previews of a successful upload. A PDS stores new blobs
    /// temporarily and may not serve them through `getBlob` until a record
    /// references them. Use the metadata-free JPEG we just uploaded instead.
    /// The existing image-cache TTL bounds abandoned previews; the DID in
    /// each key keeps unpublished images scoped to their author.
    pub async fn cache_uploaded_photo(
        &self,
        identity: &eaten_at_atproto::identity::Identity,
        cid: &str,
        jpeg: Vec<u8>,
    ) -> Result<(), AppError> {
        let renditions = tokio::task::spawn_blocking(move || {
            let image = decode(&jpeg)?;
            [PhotoSize::Thumb, PhotoSize::Full, PhotoSize::Card]
                .into_iter()
                .map(|size| encode_photo(&image, size).map(|bytes| (size, bytes)))
                .collect::<Result<Vec<_>, ImageError>>()
        })
        .await
        .map_err(|err| AppError::Upstream(err.to_string()))?
        .map_err(|err| AppError::Upstream(err.to_string()))?;
        for (size, bytes) in renditions {
            let key = own_photo_key(identity, cid, size);
            self.cache().put_bytes(Namespace::Image, &key, &bytes).await;
        }
        Ok(())
    }

    /// One of the author's own blobs at `size`, cached, for the editor's
    /// tiles (D37 amended); `None` when the repository has no such blob
    /// or it is not an image. Only reached signed in, for the caller's
    /// own repository.
    pub async fn own_photo_rendition(
        &self,
        identity: &eaten_at_atproto::identity::Identity,
        cid: &str,
        size: PhotoSize,
    ) -> Option<Rendition> {
        let key = own_photo_key(identity, cid, size);
        let build = || async {
            let url = self.repo_for(identity).blob_url(&identity.did, cid);
            let image = self.fetch_and_decode(url).await?;
            tokio::task::spawn_blocking(move || encode_photo(&image, size))
                .await
                .ok()
                .and_then(Result::ok)
        };
        let cached = self
            .cache()
            .get_or_fetch_bytes::<AppError, _, _>(Namespace::Image, &key, || async {
                Ok(build().await)
            })
            .await;
        match cached {
            Ok(Some(jpeg)) => Some(Rendition { jpeg }),
            Ok(None) => None,
            Err(_) => build().await.map(|jpeg| Rendition { jpeg }),
        }
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

fn own_photo_key(
    identity: &eaten_at_atproto::identity::Identity,
    cid: &str,
    size: PhotoSize,
) -> String {
    format!("{}/own/{}/{}", identity.did, cid, size.as_str())
}

/// Why an image was rejected or could not be produced.
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("unsupported or unrecognised image format")]
    Format,
    #[error("file of {0} bytes is over the {1} byte upload limit")]
    TooBig(usize, usize),
    #[error("image could not be brought under {0} bytes")]
    CannotShrink(usize),
    #[error("image dimensions {0}×{1} exceed the limit")]
    TooLarge(u32, u32),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("read error: {0}")]
    Io(#[from] std::io::Error),
}

/// A photo ready to upload: a fresh JPEG and its dimensions.
#[derive(Clone, PartialEq, Eq)]
pub struct PreparedPhoto {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl std::fmt::Debug for PreparedPhoto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedPhoto")
            .field("bytes", &self.jpeg.len())
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

/// Prepare an author's photo for the repository: decode it (orientation
/// applied), fit it to [`PHOTO_FIT_DIMENSION`], and **always** re-encode
/// it as JPEG under the blob cap. Re-encoding writes no metadata block,
/// so nothing a camera recorded, the position above all, is published.
pub fn photo_upload(bytes: &[u8]) -> Result<PreparedPhoto, ImageError> {
    if bytes.len() > MAX_PHOTO_UPLOAD_BYTES {
        return Err(ImageError::TooBig(bytes.len(), MAX_PHOTO_UPLOAD_BYTES));
    }
    let image = decode(bytes)?;
    // Smaller and coarser until it fits. A photograph fits on the first
    // try; only something like pure noise gets as far as the last.
    for limit in [PHOTO_FIT_DIMENSION, 1600, 1200, 800] {
        let (w, h) = image.dimensions();
        let fitted = if w.max(h) > limit {
            image.resize(limit, limit, FilterType::Lanczos3)
        } else {
            image.clone()
        };
        let (width, height) = fitted.dimensions();
        let rgb = fitted.to_rgb8();
        for quality in [JPEG_QUALITY, 72, 60] {
            let mut out = Cursor::new(Vec::new());
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
            rgb.write_with_encoder(encoder)?;
            let jpeg = out.into_inner();
            if jpeg.len() <= MAX_IMAGE_BLOB_BYTES {
                return Ok(PreparedPhoto {
                    jpeg,
                    width,
                    height,
                });
            }
        }
    }
    Err(ImageError::CannotShrink(MAX_IMAGE_BLOB_BYTES))
}

/// A photo at `size`: a centre-cropped square thumbnail, the full
/// rendition fitted to its long side, or the card's 3:2 crop, none of
/// them upscaled.
pub fn encode_photo(image: &DynamicImage, size: PhotoSize) -> Result<Vec<u8>, ImageError> {
    let framed = match size {
        PhotoSize::Thumb => image.resize_to_fill(THUMB_SIDE, THUMB_SIDE, FilterType::Lanczos3),
        PhotoSize::Full => {
            let (w, h) = image.dimensions();
            if w.max(h) > FULL_FIT_DIMENSION {
                image.resize(FULL_FIT_DIMENSION, FULL_FIT_DIMENSION, FilterType::Lanczos3)
            } else {
                image.clone()
            }
        }
        PhotoSize::Card => {
            let (w, h) = card_dimensions(image.dimensions());
            image.resize_to_fill(w, h, FilterType::Lanczos3)
        }
    };
    let mut out = Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
    framed.to_rgb8().write_with_encoder(encoder)?;
    Ok(out.into_inner())
}

/// The largest 3:2 frame that fits inside an image, at most
/// [`CARD_WIDTH`] wide: a wide image loses its sides, a tall one its
/// top and bottom, and a small one is cropped at its own size rather
/// than blown up.
fn card_dimensions((w, h): (u32, u32)) -> (u32, u32) {
    let mut width = w.min(CARD_WIDTH);
    let mut height = width * 2 / 3;
    if height > h {
        height = h;
        width = (h * 3 / 2).min(w);
    }
    (width.max(1), height.max(1))
}

/// Decode with format sniffed from the bytes (never from the declared
/// type), dimensions checked before pixels are allocated, and the EXIF
/// orientation applied so a phone photo comes out the way up it was
/// taken.
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
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image)
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

/// A deterministic placeholder: two-tone blocks derived from the place's
/// name, so the same place always gets the same image and different
/// places are told apart at a glance.
pub fn placeholder(visit_doc: &VisitDocument) -> DynamicImage {
    placeholder_from_seed(&visit_doc.visit.place.name)
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

    /// A JPEG with an EXIF orientation of 6 (rotate 90° clockwise to
    /// view) wrapped around `image`.
    fn jpeg_with_orientation_6(image: &RgbImage) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90);
        image.write_with_encoder(encoder).unwrap();
        let jpeg = out.into_inner();
        // Minimal EXIF (big-endian): one IFD entry, tag 0x0112, SHORT, 6.
        let tiff: Vec<u8> = [
            b"MM\x00\x2a\x00\x00\x00\x08".to_vec(),
            b"\x00\x01".to_vec(),
            b"\x01\x12\x00\x03\x00\x00\x00\x01\x00\x06\x00\x00".to_vec(),
            b"\x00\x00\x00\x00".to_vec(),
        ]
        .concat();
        let mut app1 = b"Exif\x00\x00".to_vec();
        app1.extend(tiff);
        let len = u16::try_from(app1.len() + 2).unwrap();
        let mut with = jpeg[..2].to_vec();
        with.extend([0xff, 0xe1]);
        with.extend(len.to_be_bytes());
        with.extend(app1);
        with.extend(&jpeg[2..]);
        with
    }

    fn has_app1(jpeg: &[u8]) -> bool {
        jpeg.windows(2).any(|w| w == [0xff, 0xe1])
    }

    #[test]
    fn photos_are_re_encoded_upright_and_without_metadata() {
        // 300 wide by 100 tall, stored with "rotate to view": upright it
        // is 100 wide by 300 tall.
        let wide = RgbImage::from_pixel(300, 100, Rgb([200, 30, 30]));
        let stored = jpeg_with_orientation_6(&wide);
        assert!(has_app1(&stored));
        let photo = photo_upload(&stored).unwrap();
        assert_eq!((photo.width, photo.height), (100, 300));
        assert!(!has_app1(&photo.jpeg), "no EXIF survives");
        assert_eq!(decode(&photo.jpeg).unwrap().dimensions(), (100, 300));

        // A PNG comes out as JPEG too, at the same size when small.
        let photo = photo_upload(&png(64, 48)).unwrap();
        assert_eq!(&photo.jpeg[..2], &[0xff, 0xd8]);
        assert_eq!((photo.width, photo.height), (64, 48));

        // A big one is fitted to the long side and under the cap.
        let big = decode(&png(3000, 2000)).unwrap();
        let mut out = Cursor::new(Vec::new());
        big.write_to(&mut out, ImageFormat::Png).unwrap();
        let photo = photo_upload(&out.into_inner()).unwrap();
        assert_eq!((photo.width, photo.height), (2048, 1365));
        assert!(photo.jpeg.len() <= MAX_IMAGE_BLOB_BYTES);

        assert!(matches!(
            photo_upload(&vec![0; MAX_PHOTO_UPLOAD_BYTES + 1]),
            Err(ImageError::TooBig(..))
        ));
        assert!(matches!(
            photo_upload(b"not an image"),
            Err(ImageError::Format)
        ));
    }

    #[test]
    fn photo_renditions_are_a_square_thumb_and_a_fitted_full() {
        let image = decode(&png(1200, 600)).unwrap();
        let thumb = decode(&encode_photo(&image, PhotoSize::Thumb).unwrap()).unwrap();
        assert_eq!(thumb.dimensions(), (400, 400));
        let full = decode(&encode_photo(&image, PhotoSize::Full).unwrap()).unwrap();
        assert_eq!(full.dimensions(), (1200, 600), "not upscaled");
        let large = decode(&png(3200, 1600)).unwrap();
        let full = decode(&encode_photo(&large, PhotoSize::Full).unwrap()).unwrap();
        assert_eq!(full.dimensions(), (1600, 800));
        assert_eq!(PhotoSize::from_query(Some("full")), PhotoSize::Full);
        assert_eq!(PhotoSize::from_query(Some("card")), PhotoSize::Card);
        assert_eq!(PhotoSize::from_query(None), PhotoSize::Thumb);
    }

    #[test]
    fn the_card_is_a_three_by_two_crop_never_upscaled() {
        // Wide: the height rules; tall: the width rules; big: capped.
        assert_eq!(card_dimensions((1200, 600)), (900, 600));
        assert_eq!(card_dimensions((480, 640)), (480, 320));
        assert_eq!(card_dimensions((3200, 1600)), (960, 640));
        assert_eq!(card_dimensions((960, 640)), (960, 640));
        assert_eq!(card_dimensions((1, 1)), (1, 1));
        let card =
            decode(&encode_photo(&decode(&png(1200, 600)).unwrap(), PhotoSize::Card).unwrap())
                .unwrap();
        assert_eq!(card.dimensions(), (900, 600));
        let portrait =
            decode(&encode_photo(&decode(&png(480, 640)).unwrap(), PhotoSize::Card).unwrap())
                .unwrap();
        assert_eq!(portrait.dimensions(), (480, 320));
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
        let a = encode(&placeholder_from_seed("Sample Place"), Size::Card).unwrap();
        let a2 = encode(&placeholder_from_seed("Sample Place"), Size::Card).unwrap();
        let b = encode(&placeholder_from_seed("Second Place"), Size::Card).unwrap();
        assert_eq!(a, a2);
        assert_ne!(a, b);
        assert_eq!(decode(&a).unwrap().dimensions(), (600, 600));
    }
}
