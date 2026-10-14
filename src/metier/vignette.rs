//! Vignette d'une photo : calculée par le serveur au dépôt, pour que l'accueil et la fiche
//! restent légers sur un téléphone (décidé le 14/10/2026). L'original reste tel quel à côté.
//!
//! Lecture JPEG, PNG ou WebP ; orientation de la prise de vue respectée (une photo prise en
//! portrait reste en portrait) ; écriture en JPEG, au plus [`COTE_MAX`] pixels de côté.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageDecoder, ImageReader};

/// Le plus grand côté de la vignette. 640 px remplissent une carte sur un écran dense, pour
/// quelques dizaines de kilo-octets.
pub const COTE_MAX: u32 = 640;

/// Qualité JPEG de la vignette : indiscernable de 90 sur une vignette, deux fois plus légère.
const QUALITE: u8 = 82;

/// Lit l'image et renvoie sa vignette en JPEG. `Err` si les octets ne sont pas une image lisible.
/// Travail de calcul : à appeler hors du fil d'exécution asynchrone (`spawn_blocking`).
pub fn creer(octets: &[u8]) -> Result<Vec<u8>, image::ImageError> {
    let lecteur = ImageReader::new(Cursor::new(octets)).with_guessed_format()?;
    let mut decodeur = lecteur.into_decoder()?;
    let orientation = decodeur.orientation()?;
    let mut image = DynamicImage::from_decoder(decodeur)?;
    image.apply_orientation(orientation);
    // `thumbnail` agrandit aussi : une petite image reste telle quelle.
    let vignette = if image.width().max(image.height()) > COTE_MAX {
        image.thumbnail(COTE_MAX, COTE_MAX).to_rgb8()
    } else {
        image.to_rgb8()
    };
    let mut sortie = Vec::new();
    vignette.write_with_encoder(JpegEncoder::new_with_quality(&mut sortie, QUALITE))?;
    Ok(sortie)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage};

    fn png(largeur: u32, hauteur: u32) -> Vec<u8> {
        let image = RgbImage::from_fn(largeur, hauteur, |x, y| Rgb([x as u8, y as u8, 90]));
        let mut tampon = Cursor::new(Vec::new());
        image.write_to(&mut tampon, ImageFormat::Png).unwrap();
        tampon.into_inner()
    }

    #[test]
    fn la_vignette_tient_dans_le_cote_maximal_et_garde_les_proportions() {
        let vignette = creer(&png(1600, 900)).unwrap();
        let lue = image::load_from_memory_with_format(&vignette, ImageFormat::Jpeg).unwrap();
        assert_eq!((lue.width(), lue.height()), (640, 360));

        // Une petite image n'est pas agrandie.
        let vignette = creer(&png(300, 200)).unwrap();
        let lue = image::load_from_memory_with_format(&vignette, ImageFormat::Jpeg).unwrap();
        assert_eq!((lue.width(), lue.height()), (300, 200));
    }

    #[test]
    fn des_octets_qui_ne_sont_pas_une_image_sont_refuses() {
        assert!(creer(b"%PDF-1.4 pas une image").is_err());
        assert!(creer(b"").is_err());
    }
}
