//! Documents : dépôt, téléchargement et suppression des fichiers rattachés.
//!
//! Le fichier est écrit sous `DOCUMENTS_DIR` avec un nom généré (UUID + extension nettoyée).
//! Le nom d'origine n'est jamais utilisé comme chemin : il pourrait contenir `../` ou des
//! caractères interdits. Seul le nom généré est stocké en base.

use std::path::{Path, PathBuf};

use sqlx::PgPool;

use crate::commun::types::{Choix, TypeDocument};
use crate::donnees::documents::{self, est_image_mime, Document, NouveauDocument};
use crate::erreurs::{a_le_code, ErreurApp, Resultat, CODE_VIOLATION_CLE_ETRANGERE};
use crate::metier::{validation, vignette};

/// Taille maximale d'un fichier déposé.
pub const TAILLE_MAX_OCTETS: usize = 10 * 1024 * 1024;

/// Message de refus d'un fichier trop gros. Il sert à l'identique à trois endroits : dans le
/// navigateur dès que le fichier est choisi (`static/app.js`), pendant l'envoi quand la couche
/// `web` cesse de lire, et au contrôle fait par [`deposer`].
pub fn message_trop_volumineux() -> String {
    format!(
        "Le fichier dépasse la taille maximale de {} Mo.",
        TAILLE_MAX_OCTETS / (1024 * 1024)
    )
}

/// Refus d'une « photo du véhicule » qui n'est pas une image.
pub fn message_photo_pas_une_image() -> &'static str {
    "Une photo du véhicule est une image (JPEG, PNG ou WebP). Pour un autre fichier, choisissez un autre type."
}

/// Refus d'une image que le serveur n'a pas su lire.
pub fn message_photo_illisible() -> &'static str {
    "La photo n'a pas pu être lue. Reprenez-la, ou déposez-la en JPEG."
}

/// À quoi le document est rattaché.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rattachement {
    Vehicule(i64),
    Conducteur(i64),
}

impl Rattachement {
    /// Véhicule choisi dans la liste déroulante de la page « Déposer un document ».
    pub fn vehicule_choisi(valeur: &str) -> Resultat<Rattachement> {
        validation::identifiant(valeur, "véhicule").map(Rattachement::Vehicule)
    }
}

/// Formulaire de dépôt tel qu'il arrive : que des valeurs brutes, validées par [`deposer`].
#[derive(Debug, Clone, Default)]
pub struct SaisieDocument {
    pub type_document: String,
    /// Nom du fichier sur l'appareil de l'utilisateur. Vide si aucun fichier n'a été choisi.
    pub nom_original: String,
    pub type_mime: String,
    pub contenu: Vec<u8>,
}

/// Types proposés pour un véhicule, dans l'ordre de la liste déroulante.
///
/// Cette liste et celle des conducteurs sont un confort d'écran : [`deposer`] accepte tout type
/// du vocabulaire, quel que soit le rattachement.
pub fn types_pour_vehicule() -> Vec<Choix> {
    choix(&[
        TypeDocument::PhotoVehicule,
        TypeDocument::Facture,
        TypeDocument::ControleTechnique,
        TypeDocument::CarteGrise,
        TypeDocument::Assurance,
        TypeDocument::Autre,
    ])
}

/// Types proposés pour un conducteur.
pub fn types_pour_conducteur() -> Vec<Choix> {
    choix(&[TypeDocument::Permis, TypeDocument::Autre])
}

fn choix(types: &[TypeDocument]) -> Vec<Choix> {
    types
        .iter()
        .map(|t| Choix {
            code: t.code(),
            libelle: t.libelle(),
        })
        .collect()
}

/// Extension sûre : lettres et chiffres ASCII seulement, 10 caractères au plus, en minuscules.
/// Tout autre cas donne `bin`.
pub fn extension_sure(nom_original: &str) -> String {
    let extension = Path::new(nom_original)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if extension.is_empty()
        || extension.len() > 10
        || !extension.chars().all(|c| c.is_ascii_alphanumeric())
    {
        "bin".to_string()
    } else {
        extension.to_ascii_lowercase()
    }
}

/// Nom d'origine nettoyé pour l'en-tête de téléchargement : ni guillemet, ni retour à la ligne,
/// ni séparateur de chemin.
pub fn nom_telechargement(nom_original: &str) -> String {
    let propre: String = nom_original
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '"' | '\\' | '/' | ';'))
        .take(150)
        .collect();
    if propre.trim().is_empty() {
        "document".to_string()
    } else {
        propre
    }
}

pub fn chemin_complet(documents_dir: &Path, document: &Document) -> PathBuf {
    documents_dir.join(&document.chemin)
}

pub async fn deposer(
    pool: &PgPool,
    documents_dir: &Path,
    rattachement: Rattachement,
    saisie: &SaisieDocument,
) -> Resultat<Document> {
    let nom_original = saisie.nom_original.trim();
    let type_mime = saisie.type_mime.trim();
    let contenu = saisie.contenu.as_slice();
    if nom_original.is_empty() {
        return Err(ErreurApp::metier("Aucun fichier n'a été choisi."));
    }
    if contenu.is_empty() {
        return Err(ErreurApp::metier("Le fichier est vide."));
    }
    if contenu.len() > TAILLE_MAX_OCTETS {
        return Err(ErreurApp::metier(message_trop_volumineux()));
    }
    if saisie.type_document.trim().is_empty() {
        return Err(ErreurApp::metier("Choisissez le type du document."));
    }
    let type_document = validation::code_parmi(
        &saisie.type_document,
        "type de document",
        TypeDocument::depuis_code,
    )?;

    // La photo du véhicule a sa vignette, calculée ici, avant d'écrire quoi que ce soit : une
    // image illisible est refusée comme un fichier trop gros, sans rien laisser sur le disque.
    let octets_vignette = if type_document == TypeDocument::PhotoVehicule {
        if !est_image_mime(type_mime) {
            return Err(ErreurApp::metier(message_photo_pas_une_image()));
        }
        let contenu = contenu.to_vec();
        let resultat = tokio::task::spawn_blocking(move || vignette::creer(&contenu))
            .await
            .map_err(std::io::Error::other)?;
        Some(resultat.map_err(|erreur| {
            tracing::warn!(%erreur, "photo du véhicule illisible");
            ErreurApp::metier(message_photo_illisible())
        })?)
    } else {
        None
    };

    tokio::fs::create_dir_all(documents_dir).await?;
    let base = uuid::Uuid::new_v4();
    let chemin = format!("{base}.{}", extension_sure(nom_original));
    let complet = documents_dir.join(&chemin);
    tokio::fs::write(&complet, contenu).await?;
    let mut vignette = String::new();
    if let Some(octets) = octets_vignette {
        vignette = format!("{base}.vignette.jpg");
        tokio::fs::write(documents_dir.join(&vignette), octets).await?;
    }

    let (vehicule_id, conducteur_id) = match rattachement {
        Rattachement::Vehicule(id) => (Some(id), None),
        Rattachement::Conducteur(id) => (None, Some(id)),
    };
    let nouveau = NouveauDocument {
        vehicule_id,
        conducteur_id,
        type_document,
        nom_original: nom_original.chars().take(200).collect(),
        chemin,
        type_mime: if type_mime.is_empty() {
            "application/octet-stream".to_string()
        } else {
            type_mime.chars().take(100).collect()
        },
        taille_octets: contenu.len() as i64,
        vignette,
    };
    match documents::inserer(pool, &nouveau).await {
        Ok(document) => Ok(document),
        Err(erreur) => {
            // La base a refusé : on ne laisse pas de fichier orphelin sur le disque.
            let _ = tokio::fs::remove_file(&complet).await;
            if !nouveau.vignette.is_empty() {
                let _ = tokio::fs::remove_file(documents_dir.join(&nouveau.vignette)).await;
            }
            Err(if a_le_code(&erreur, CODE_VIOLATION_CLE_ETRANGERE) {
                ErreurApp::metier("Le véhicule ou le conducteur n'existe plus.")
            } else {
                erreur.into()
            })
        }
    }
}

pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<Document> {
    documents::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)
}

/// Métadonnées et contenu du fichier.
pub async fn lire(pool: &PgPool, documents_dir: &Path, id: i64) -> Resultat<(Document, Vec<u8>)> {
    let document = obtenir(pool, id).await?;
    let contenu = tokio::fs::read(chemin_complet(documents_dir, &document))
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ErreurApp::Introuvable,
            _ => ErreurApp::from(e),
        })?;
    Ok((document, contenu))
}

/// La vignette d'une photo du véhicule. Introuvable pour un document qui n'en a pas.
pub async fn lire_vignette(pool: &PgPool, documents_dir: &Path, id: i64) -> Resultat<Vec<u8>> {
    let document = obtenir(pool, id).await?;
    if !document.a_vignette() {
        return Err(ErreurApp::Introuvable);
    }
    tokio::fs::read(documents_dir.join(&document.vignette))
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ErreurApp::Introuvable,
            _ => ErreurApp::from(e),
        })
}

pub async fn derniere_photo(pool: &PgPool, vehicule_id: i64) -> Resultat<Option<Document>> {
    Ok(documents::derniere_photo(pool, vehicule_id).await?)
}

pub async fn dernieres_photos(pool: &PgPool) -> Resultat<Vec<Document>> {
    Ok(documents::dernieres_photos(pool).await?)
}

pub async fn lister_par_vehicule(pool: &PgPool, vehicule_id: i64) -> Resultat<Vec<Document>> {
    Ok(documents::lister_par_vehicule(pool, vehicule_id).await?)
}

pub async fn lister_par_conducteur(pool: &PgPool, conducteur_id: i64) -> Resultat<Vec<Document>> {
    Ok(documents::lister_par_conducteur(pool, conducteur_id).await?)
}

pub async fn supprimer(pool: &PgPool, documents_dir: &Path, id: i64) -> Resultat<Document> {
    let document = obtenir(pool, id).await?;
    if !documents::supprimer(pool, id).await? {
        return Err(ErreurApp::Introuvable);
    }
    supprimer_fichiers(documents_dir, std::slice::from_ref(&document)).await;
    Ok(document)
}

/// Efface les fichiers de documents déjà supprimés en base. Un fichier déjà absent n'est pas une
/// erreur ; tout autre échec est journalisé sans interrompre l'opération, la base faisant foi.
pub async fn supprimer_fichiers(documents_dir: &Path, documents: &[Document]) {
    for document in documents {
        let mut chemins = vec![chemin_complet(documents_dir, document)];
        if document.a_vignette() {
            chemins.push(documents_dir.join(&document.vignette));
        }
        for chemin in chemins {
            if let Err(erreur) = tokio::fs::remove_file(&chemin).await {
                if erreur.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(?chemin, %erreur, "fichier de document non supprimé");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions() {
        assert_eq!(extension_sure("carte-grise.PDF"), "pdf");
        assert_eq!(extension_sure("photo.jpeg"), "jpeg");
        assert_eq!(extension_sure("sans-extension"), "bin");
        assert_eq!(extension_sure("bizarre.p df"), "bin");
        assert_eq!(extension_sure("../../etc/passwd"), "bin");
        assert_eq!(extension_sure("x.tropLongueExtension"), "bin");
    }

    #[test]
    fn nom_de_telechargement_nettoye() {
        assert_eq!(nom_telechargement("carte grise.pdf"), "carte grise.pdf");
        assert_eq!(nom_telechargement("a\"b;c/d\\e.txt"), "abcde.txt");
        assert_eq!(nom_telechargement("   "), "document");
    }

    #[test]
    fn types_proposes_selon_le_rattachement() {
        let codes = |choix: Vec<Choix>| choix.iter().map(|c| c.code).collect::<Vec<_>>();
        assert_eq!(
            codes(types_pour_vehicule()),
            [
                "photo_vehicule",
                "facture",
                "controle_technique",
                "carte_grise",
                "assurance",
                "autre"
            ]
        );
        assert_eq!(codes(types_pour_conducteur()), ["permis", "autre"]);
    }

    #[test]
    fn vehicule_choisi_dans_la_liste() {
        assert_eq!(
            Rattachement::vehicule_choisi(" 12 ").unwrap(),
            Rattachement::Vehicule(12)
        );
        assert!(Rattachement::vehicule_choisi("").is_err());
        assert!(Rattachement::vehicule_choisi("abc").is_err());
    }
}
