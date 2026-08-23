//! Normalises an imported photo so that what is stored is what the model sees.
//!
//! Measured, not assumed: given a JPEG carrying `EXIF Orientation = 6`, Claude
//! Code reads the pixels as stored and reports *"la page est couchée / pivotée
//! de 90 degrés"*. A phone writes that tag constantly, so a course photographed
//! in portrait arrives sideways and the transcription collapses — 25 % to 52 %
//! confidence, invented content, doubts blaming "l'inclinaison de la page".
//!
//! So the rotation is baked into the pixels at import, once, rather than
//! trusted to every downstream reader. The same pass caps the resolution: a
//! 48 megapixel photo is downsampled by whoever reads it anyway, and doing it
//! here means we choose the resampling instead of inheriting it.

use crate::logbus;
use image::imageops::FilterType;
use std::fs;
use std::path::Path;

/// Long edge kept after import.
///
/// Comfortably above what a vision model resolves, so nothing is lost, while
/// keeping the review panel's "compare with the photo" crisp and the workbook
/// a reasonable size.
const MAX_EDGE: u32 = 2400;

/// The eight EXIF orientations, as the transform needed to make pixels upright.
fn orientation(path: &Path) -> u32 {
    let Ok(file) = fs::File::open(path) else {
        return 1;
    };
    let mut reader = std::io::BufReader::new(file);

    exif::Reader::new()
        .read_from_container(&mut reader)
        .ok()
        .and_then(|exif| {
            exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|field| field.value.get_uint(0))
        })
        .filter(|value| (1..=8).contains(value))
        .unwrap_or(1)
}

fn upright(image: image::DynamicImage, orientation: u32) -> image::DynamicImage {
    match orientation {
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.rotate90().fliph(),
        6 => image.rotate90(),
        7 => image.rotate270().fliph(),
        8 => image.rotate270(),
        _ => image,
    }
}

/// Rotates to upright, caps the long edge, and re-encodes without metadata.
///
/// Returns whether anything actually changed, for the log.
pub fn normalise(source: &Path, destination: &Path) -> Result<bool, String> {
    let rotation = orientation(source);

    let decoded = image::open(source)
        .map_err(|e| format!("Image illisible : {e}"))?;

    let rotated = upright(decoded, rotation);
    let (width, height) = (rotated.width(), rotated.height());
    let oversized = width.max(height) > MAX_EDGE;

    let final_image = if oversized {
        rotated.resize(MAX_EDGE, MAX_EDGE, FilterType::Lanczos3)
    } else {
        rotated
    };

    // Re-encoding drops the EXIF block along the way, so no later reader can
    // rotate the image a second time.
    final_image
        .into_rgb8()
        .save(destination)
        .map_err(|e| format!("Écriture de l'image : {e}"))?;

    let changed = rotation != 1 || oversized;
    if changed {
        logbus::detail(
            "workspace",
            format!(
                "Photo normalisée — {}{}",
                if rotation != 1 { format!("redressée (EXIF {rotation})") } else { String::new() },
                if oversized {
                    format!(
                        "{}réduite de {width}×{height} à {} px de côté long",
                        if rotation != 1 { ", " } else { "" },
                        MAX_EDGE
                    )
                } else {
                    String::new()
                }
            ),
            destination.to_string_lossy().to_string(),
        );
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The failure this module exists for: a portrait page stored landscape
    /// with `Orientation = 6` must come out portrait.
    #[test]
    fn orientation_six_is_rotated_upright() {
        let wide = image::DynamicImage::new_rgb8(80, 40);
        let upright_image = upright(wide, 6);
        assert_eq!(
            (upright_image.width(), upright_image.height()),
            (40, 80),
            "an orientation-6 photo must end up portrait"
        );
    }

    #[test]
    fn every_orientation_is_handled_and_only_swaps_when_it_should() {
        for tag in 1..=8u32 {
            let result = upright(image::DynamicImage::new_rgb8(80, 40), tag);
            let swapped = matches!(tag, 5 | 6 | 7 | 8);
            let expected = if swapped { (40, 80) } else { (80, 40) };
            assert_eq!(
                (result.width(), result.height()),
                expected,
                "orientation {tag} produced the wrong shape"
            );
        }
    }

    #[test]
    fn a_large_photo_is_capped_and_stripped() {
        let dir = std::env::temp_dir().join(format!("plume-photo-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let source = dir.join("big.jpg");
        image::DynamicImage::new_rgb8(4000, 3000)
            .save(&source)
            .unwrap();

        let destination = dir.join("out.jpg");
        assert!(normalise(&source, &destination).unwrap());

        let produced = image::open(&destination).unwrap();
        assert_eq!(produced.width().max(produced.height()), MAX_EDGE);
        assert_eq!(orientation(&destination), 1, "metadata must not survive");

        let _ = fs::remove_dir_all(&dir);
    }
}
