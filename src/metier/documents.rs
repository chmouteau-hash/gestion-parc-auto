//! Documents : dépôt, téléchargement et suppression des fichiers rattachés.
//!
//! Le fichier est écrit sous `DOCUMENTS_DIR` avec un nom généré (UUID + extension nettoyée).
//! Le nom d'origine n'est jamais utilisé comme chemin : il pourrait contenir `../` ou des
//! caractères interdits. Seul le nom généré est stocké en base.

use std::path::{Path, PathBuf};

use sqlx::PgPool;

use crate::donnees::documents::{self, Document, NouveauDocument};
use crate::erreurs::{a_le_code, ErreurApp, Resultat, CODE_VIOLATION_CLE_ETRANGERE};

/// Taille maximale d'un fichier déposé.
pub const TAILLE_MAX_OCTETS: usize = 10 * 1024 * 1024;

/// À quoi le document est rattaché.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rattachement {
    Vehicule(i64),
    Conducteur(i64),
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
    nom_original: &str,
    type_mime: &str,
    contenu: &[u8],
) -> Resultat<Document> {
    let nom_original = nom_original.trim();
    if nom_original.is_empty() {
        return Err(ErreurApp::metier("Aucun fichier n'a été choisi."));
    }
    if contenu.is_empty() {
        return Err(ErreurApp::metier("Le fichier est vide."));
    }
    if contenu.len() > TAILLE_MAX_OCTETS {
        return Err(ErreurApp::metier(format!(
            "Le fichier dépasse la taille maximale de {} Mo.",
            TAILLE_MAX_OCTETS / (1024 * 1024)
        )));
    }

    tokio::fs::create_dir_all(documents_dir).await?;
    let chemin = format!("{}.{}", uuid::Uuid::new_v4(), extension_sure(nom_original));
    let complet = documents_dir.join(&chemin);
    tokio::fs::write(&complet, contenu).await?;

    let (vehicule_id, conducteur_id) = match rattachement {
        Rattachement::Vehicule(id) => (Some(id), None),
        Rattachement::Conducteur(id) => (None, Some(id)),
    };
    let nouveau = NouveauDocument {
        vehicule_id,
        conducteur_id,
        nom_original: nom_original.chars().take(200).collect(),
        chemin,
        type_mime: if type_mime.trim().is_empty() {
            "application/octet-stream".to_string()
        } else {
            type_mime.trim().chars().take(100).collect()
        },
        taille_octets: contenu.len() as i64,
    };
    match documents::inserer(pool, &nouveau).await {
        Ok(document) => Ok(document),
        Err(erreur) => {
            // La base a refusé : on ne laisse pas de fichier orphelin sur le disque.
            let _ = tokio::fs::remove_file(&complet).await;
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
        let chemin = chemin_complet(documents_dir, document);
        if let Err(erreur) = tokio::fs::remove_file(&chemin).await {
            if erreur.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(?chemin, %erreur, "fichier de document non supprimé");
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
}
