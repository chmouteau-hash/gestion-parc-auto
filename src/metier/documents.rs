//! Documents : dépôt, téléchargement et suppression des fichiers rattachés.
//!
//! Le fichier est écrit sous `DOCUMENTS_DIR` avec un nom généré (UUID + extension nettoyée).
//! Le nom d'origine n'est jamais utilisé comme chemin : il pourrait contenir `../` ou des
//! caractères interdits. Seul le nom généré est stocké en base.

use std::path::{Path, PathBuf};

use sqlx::PgPool;

use crate::commun::types::{Choix, Dossier, TypeDocument};
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
        TypeDocument::FactureAchat,
        TypeDocument::CertificatCession,
        TypeDocument::ContratAchat,
        TypeDocument::Garantie,
        TypeDocument::Autre,
    ])
}

/// Les types d'un dossier de la fiche (#109) : la liste que propose « Ajouter » depuis ce
/// dossier. Le premier est celui pré-choisi.
pub fn types_du_dossier(dossier: Dossier) -> Vec<Choix> {
    choix(&dossier.types())
}

/// Un dossier de la section « Documents » de la fiche véhicule, avec ses documents.
#[derive(Debug)]
pub struct DossierDeDocuments {
    pub dossier: Dossier,
    pub documents: Vec<Document>,
}

impl DossierDeDocuments {
    pub fn code(&self) -> &'static str {
        self.dossier.code()
    }

    pub fn libelle(&self) -> &'static str {
        self.dossier.libelle()
    }
}

/// Range les documents d'un véhicule par dossier (#109), tous les dossiers dans l'ordre
/// d'affichage, vides compris : la fiche décide ce qu'elle montre. Les photos du véhicule n'ont
/// pas de dossier et sont laissées de côté ; un type inconnu de la base va dans « Autres ». L'ordre
/// des documents est conservé.
pub fn par_dossier(documents: Vec<Document>) -> Vec<DossierDeDocuments> {
    let mut dossiers: Vec<DossierDeDocuments> = Dossier::TOUS
        .iter()
        .map(|&dossier| DossierDeDocuments {
            dossier,
            documents: Vec::new(),
        })
        .collect();
    for document in documents {
        let dossier = match TypeDocument::depuis_code(&document.type_document) {
            Some(t) => t.dossier(),
            None => Some(Dossier::Autres),
        };
        if let Some(dossier) = dossier {
            if let Some(d) = dossiers.iter_mut().find(|d| d.dossier == dossier) {
                d.documents.push(document);
            }
        }
    }
    dossiers
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

/// Sépare un nom de fichier de son extension : `("CT 2025", ".jpg")`. L'extension est ce que
/// [`extension_sure`] accepte (lettres et chiffres ASCII, 10 au plus) ; sinon, ou sans point, le
/// nom entier est la racine et l'extension est vide.
pub fn separer_extension(nom: &str) -> (&str, &str) {
    match nom.rfind('.') {
        Some(i)
            if i > 0
                && i + 1 < nom.len()
                && nom.len() - i <= 11
                && nom[i + 1..].chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            (&nom[..i], &nom[i..])
        }
        _ => (nom, ""),
    }
}

/// Le nom tapé, rendu sûr pour un nom de fichier : `/`, `\` et `:` deviennent des tirets (on tape
/// volontiers une date « 12/03/2025 »), les caractères que Windows interdit et les caractères de
/// contrôle disparaissent, les blancs se resserrent, les points et blancs du bord tombent.
fn nom_propre(saisie: &str) -> String {
    let remplace: String = saisie
        .chars()
        .filter_map(|c| match c {
            '/' | '\\' | ':' => Some('-'),
            c if c.is_control() || matches!(c, '*' | '?' | '"' | '<' | '>' | '|' | ';') => None,
            c => Some(c),
        })
        .collect();
    let resserre = remplace.split_whitespace().collect::<Vec<_>>().join(" ");
    resserre
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .to_string()
}

/// Renomme un document : le nom change à l'écran et au téléchargement, le fichier sur disque non.
/// On ne saisit que la racine ; l'extension du fichier d'origine est conservée, car le type du
/// fichier ne change pas (une extension tapée en trop est retirée plutôt que doublée).
pub async fn renommer(pool: &PgPool, id: i64, saisie: &str) -> Resultat<Document> {
    let document = obtenir(pool, id).await?;
    let (_, extension) = separer_extension(&document.nom_original);
    let mut racine = nom_propre(saisie);
    if !extension.is_empty() && racine.to_lowercase().ends_with(&extension.to_lowercase()) {
        let fin = racine.len() - extension.len();
        racine = racine[..fin].trim_end().to_string();
    }
    if racine.is_empty() {
        return Err(ErreurApp::metier(
            "Le nom du document ne peut pas être vide.",
        ));
    }
    let nom = format!("{racine}{extension}");
    if nom.chars().count() > validation::LONGUEUR_MAX_TEXTE {
        return Err(ErreurApp::metier(format!(
            "Le nom du document dépasse {} caractères (extension « {extension} » comprise).",
            validation::LONGUEUR_MAX_TEXTE
        )));
    }
    documents::renommer(pool, id, &nom)
        .await?
        .ok_or(ErreurApp::Introuvable)
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

/// L'en-tête `Content-Disposition` d'un document : `inline` pour l'afficher, `attachment` pour le
/// télécharger, avec son nom. Un nom ASCII s'écrit tel quel. Un nom accentué (« Contrôle
/// technique ») s'écrit aussi en `filename*=UTF-8''…` (RFC 6266) : sans cela, selon le navigateur,
/// les accents arrivent abîmés. `filename` garde alors une version ASCII pour les plus anciens.
pub fn en_tete_disposition(mode: &str, nom_original: &str) -> String {
    let nom = nom_telechargement(nom_original);
    if nom.is_ascii() {
        return format!("{mode}; filename=\"{nom}\"");
    }
    let repli: String = nom
        .chars()
        .map(|c| if c.is_ascii() { c } else { '_' })
        .collect();
    let mut code = String::new();
    for octet in nom.bytes() {
        if octet.is_ascii_alphanumeric() || matches!(octet, b'-' | b'.' | b'_' | b'~') {
            code.push(char::from(octet));
        } else {
            code.push_str(&format!("%{octet:02X}"));
        }
    }
    format!("{mode}; filename=\"{repli}\"; filename*=UTF-8''{code}")
}

pub fn chemin_complet(documents_dir: &Path, document: &Document) -> PathBuf {
    documents_dir.join(&document.chemin)
}

/// Un dépôt contrôlé et prêt à être écrit : la saisie a passé [`preparer`], la vignette d'une
/// photo est calculée. Rien n'est encore sur le disque ni en base.
///
/// Les deux temps sont séparés pour la création d'un véhicule avec sa photo : la photo est
/// contrôlée avant que le véhicule n'existe, et un refus ne laisse rien derrière lui.
#[derive(Debug)]
pub struct DepotPrepare {
    type_document: TypeDocument,
    nom_original: String,
    type_mime: String,
    contenu: Vec<u8>,
    /// La vignette, pour une photo du véhicule.
    vignette: Option<Vec<u8>>,
    /// L'intervention à laquelle relier le document (sa facture, #100).
    intervention_id: Option<i64>,
}

impl DepotPrepare {
    /// Relie le document à une intervention du véhicule : sa facture (#100). L'appelant s'est
    /// assuré que l'intervention est bien celle du véhicule du rattachement.
    pub fn pour_intervention(mut self, intervention_id: i64) -> DepotPrepare {
        self.intervention_id = Some(intervention_id);
        self
    }
}

/// Contrôle la saisie et calcule la vignette d'une photo, sans rien écrire.
pub async fn preparer(saisie: &SaisieDocument) -> Resultat<DepotPrepare> {
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
    let vignette = if type_document == TypeDocument::PhotoVehicule {
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

    Ok(DepotPrepare {
        type_document,
        nom_original: nom_original.chars().take(200).collect(),
        type_mime: if type_mime.is_empty() {
            "application/octet-stream".to_string()
        } else {
            type_mime.chars().take(100).collect()
        },
        contenu: contenu.to_vec(),
        vignette,
        intervention_id: None,
    })
}

/// Contrôle puis écrit : [`preparer`] et [`ecrire`] en un appel.
pub async fn deposer(
    pool: &PgPool,
    documents_dir: &Path,
    rattachement: Rattachement,
    saisie: &SaisieDocument,
) -> Resultat<Document> {
    let prepare = preparer(saisie).await?;
    ecrire(pool, documents_dir, rattachement, prepare).await
}

/// Contrôle puis écrit un document relié à une intervention du véhicule : sa facture (#100).
pub async fn deposer_pour_intervention(
    pool: &PgPool,
    documents_dir: &Path,
    vehicule_id: i64,
    intervention_id: i64,
    saisie: &SaisieDocument,
) -> Resultat<Document> {
    let prepare = preparer(saisie).await?.pour_intervention(intervention_id);
    ecrire(
        pool,
        documents_dir,
        Rattachement::Vehicule(vehicule_id),
        prepare,
    )
    .await
}

/// Écrit le fichier (et la vignette), puis enregistre le document en base.
pub async fn ecrire(
    pool: &PgPool,
    documents_dir: &Path,
    rattachement: Rattachement,
    prepare: DepotPrepare,
) -> Resultat<Document> {
    tokio::fs::create_dir_all(documents_dir).await?;
    let base = uuid::Uuid::new_v4();
    let chemin = format!("{base}.{}", extension_sure(&prepare.nom_original));
    let complet = documents_dir.join(&chemin);
    tokio::fs::write(&complet, &prepare.contenu).await?;
    let mut vignette = String::new();
    if let Some(octets) = &prepare.vignette {
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
        intervention_id: prepare.intervention_id,
        type_document: prepare.type_document,
        nom_original: prepare.nom_original,
        chemin,
        type_mime: prepare.type_mime,
        taille_octets: prepare.contenu.len() as i64,
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

/// La photo de la fiche d'un véhicule : celle qu'on a choisie, à défaut la plus récente.
pub async fn photo_de_la_fiche(pool: &PgPool, vehicule_id: i64) -> Resultat<Option<Document>> {
    Ok(documents::photo_de_la_fiche(pool, vehicule_id).await?)
}

/// La photo de la fiche de chaque véhicule qui en a une.
pub async fn photos_des_fiches(pool: &PgPool) -> Resultat<Vec<Document>> {
    Ok(documents::photos_des_fiches(pool).await?)
}

/// Toutes les photos d'un véhicule, la photo de la fiche en premier.
pub async fn photos_du_vehicule(pool: &PgPool, vehicule_id: i64) -> Resultat<Vec<Document>> {
    Ok(documents::photos_du_vehicule(pool, vehicule_id).await?)
}

/// Choisit la photo qui figure en tête de la fiche de son véhicule et sur sa carte d'accueil
/// (#66). Le choix précédent est retiré dans la même transaction : un véhicule n'en a qu'un.
/// Introuvable pour un document qui n'est pas une photo du véhicule (avec sa vignette).
pub async fn mettre_en_tete(pool: &PgPool, id: i64) -> Resultat<Document> {
    let document = obtenir(pool, id).await?;
    let vehicule_id = document.vehicule_id.ok_or(ErreurApp::Introuvable)?;
    let mut transaction = pool.begin().await?;
    documents::retirer_la_photo_de_tete(&mut *transaction, vehicule_id).await?;
    if !documents::poser_la_photo_de_tete(&mut *transaction, vehicule_id, id).await? {
        return Err(ErreurApp::Introuvable);
    }
    transaction.commit().await?;
    Ok(Document {
        photo_de_tete: true,
        ..document
    })
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
    fn l_extension_se_separe_de_la_racine() {
        assert_eq!(separer_extension("17921513.jpg"), ("17921513", ".jpg"));
        assert_eq!(
            separer_extension("CT 12.03.2025.pdf"),
            ("CT 12.03.2025", ".pdf")
        );
        assert_eq!(separer_extension("sans-extension"), ("sans-extension", ""));
        assert_eq!(
            separer_extension("fini-par-un-point."),
            ("fini-par-un-point.", "")
        );
        assert_eq!(separer_extension(".cache"), (".cache", ""));
        assert_eq!(separer_extension("x.p df"), ("x.p df", ""));
        assert_eq!(
            separer_extension("x.tropLongueExtension"),
            ("x.tropLongueExtension", "")
        );
    }

    #[test]
    fn le_nom_tape_est_rendu_sur() {
        assert_eq!(nom_propre("  Contrôle   technique  "), "Contrôle technique");
        assert_eq!(nom_propre("CT 12/03/2025"), "CT 12-03-2025");
        assert_eq!(nom_propre("a\\b:c"), "a-b-c");
        assert_eq!(
            nom_propre("Facture <n°1> \"vidange\" ?*|;"),
            "Facture n°1 vidange"
        );
        assert_eq!(nom_propre("..caché.."), "caché");
        assert_eq!(nom_propre("   "), "");
        assert_eq!(nom_propre("*?"), "");
    }

    #[test]
    fn l_en_tete_de_telechargement_dit_le_nom_accentue() {
        assert_eq!(
            en_tete_disposition("inline", "image.jpg"),
            "inline; filename=\"image.jpg\""
        );
        assert_eq!(
            en_tete_disposition("attachment", "Contrôle 12-03.pdf"),
            concat!(
                "attachment; filename=\"Contr_le 12-03.pdf\"; ",
                "filename*=UTF-8''Contr%C3%B4le%2012-03.pdf"
            )
        );
    }

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
                "facture_achat",
                "certificat_cession",
                "contrat_achat",
                "garantie",
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
