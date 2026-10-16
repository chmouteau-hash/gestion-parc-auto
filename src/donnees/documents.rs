//! Table `documents` : métadonnées des fichiers déposés. Le contenu est sur disque.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::commun::format::{date_heure_fr, taille_fr};
use crate::commun::types::TypeDocument;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Document {
    pub id: i64,
    pub vehicule_id: Option<i64>,
    pub conducteur_id: Option<i64>,
    /// L'intervention que ce document justifie (sa facture), quand il a été joint depuis le
    /// formulaire d'intervention (#100). Toujours un document du véhicule ; reste quand
    /// l'intervention est supprimée (`ON DELETE SET NULL`).
    pub intervention_id: Option<i64>,
    pub type_document: String,
    /// Nom donné par l'utilisateur, pour l'affichage et le téléchargement.
    pub nom_original: String,
    /// Nom du fichier dans `DOCUMENTS_DIR` (un UUID + extension), jamais le nom d'origine. Un
    /// nom interne : il ne sort pas dans l'API JSON.
    #[serde(skip)]
    pub chemin: String,
    pub type_mime: String,
    pub taille_octets: i64,
    /// Nom du fichier de la vignette dans `DOCUMENTS_DIR`, vide quand il n'y en a pas (tout
    /// document qui n'est pas une photo du véhicule). Interne, comme `chemin`.
    #[serde(skip)]
    pub vignette: String,
    /// La photo choisie pour la fiche et la carte d'accueil du véhicule (#66). Au plus une par
    /// véhicule ; sans choix, c'est la plus récente qui sert.
    pub photo_de_tete: bool,
    pub cree_le: DateTime<Utc>,
}

/// Vrai pour un type MIME d'image que le navigateur affiche et que le serveur sait lire.
pub fn est_image_mime(type_mime: &str) -> bool {
    matches!(
        type_mime,
        "image/jpeg" | "image/png" | "image/webp" | "image/gif"
    )
}

impl Document {
    pub fn a_vignette(&self) -> bool {
        !self.vignette.is_empty()
    }

    pub fn type_libelle(&self) -> &str {
        match TypeDocument::depuis_code(&self.type_document) {
            Some(valeur) => valeur.libelle(),
            None => &self.type_document,
        }
    }

    /// Vrai pour une image ou un PDF : le navigateur peut l'afficher lui-même. Tout autre
    /// fichier (HTML, SVG…) ne se propose qu'en téléchargement, pour ne jamais s'exécuter dans
    /// le contexte du site.
    pub fn est_affichable(&self) -> bool {
        self.est_image() || self.est_pdf()
    }

    /// Une image : la page du document la montre et l'imprime.
    pub fn est_image(&self) -> bool {
        est_image_mime(&self.type_mime)
    }

    /// Un PDF : le navigateur l'ouvre avec son propre lecteur, dans un autre onglet.
    pub fn est_pdf(&self) -> bool {
        self.type_mime == "application/pdf"
    }

    pub fn taille_fr(&self) -> String {
        taille_fr(self.taille_octets)
    }

    pub fn cree_le_fr(&self) -> String {
        date_heure_fr(self.cree_le)
    }
}

#[derive(Debug, Clone)]
pub struct NouveauDocument {
    pub vehicule_id: Option<i64>,
    pub conducteur_id: Option<i64>,
    pub intervention_id: Option<i64>,
    pub type_document: TypeDocument,
    pub nom_original: String,
    pub chemin: String,
    pub type_mime: String,
    pub taille_octets: i64,
    pub vignette: String,
}

/// La photo de la fiche : celle qu'on a choisie, à défaut la plus récente.
pub async fn photo_de_la_fiche(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
) -> sqlx::Result<Option<Document>> {
    sqlx::query_as::<_, Document>(
        "SELECT * FROM documents
         WHERE vehicule_id = $1 AND type_document = 'photo_vehicule' AND vignette <> ''
         ORDER BY photo_de_tete DESC, cree_le DESC, id DESC LIMIT 1",
    )
    .bind(vehicule_id)
    .fetch_optional(ex)
    .await
}

/// La photo de la fiche de chaque véhicule qui en a une (pour l'accueil : une requête, pas une
/// par carte).
pub async fn photos_des_fiches(ex: impl PgExecutor<'_>) -> sqlx::Result<Vec<Document>> {
    sqlx::query_as::<_, Document>(
        "SELECT DISTINCT ON (vehicule_id) * FROM documents
         WHERE vehicule_id IS NOT NULL AND type_document = 'photo_vehicule' AND vignette <> ''
         ORDER BY vehicule_id, photo_de_tete DESC, cree_le DESC, id DESC",
    )
    .fetch_all(ex)
    .await
}

/// Toutes les photos d'un véhicule, la photo de la fiche en premier, puis de la plus récente à
/// la plus ancienne.
pub async fn photos_du_vehicule(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
) -> sqlx::Result<Vec<Document>> {
    sqlx::query_as::<_, Document>(
        "SELECT * FROM documents
         WHERE vehicule_id = $1 AND type_document = 'photo_vehicule' AND vignette <> ''
         ORDER BY photo_de_tete DESC, cree_le DESC, id DESC",
    )
    .bind(vehicule_id)
    .fetch_all(ex)
    .await
}

pub async fn obtenir(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Document>> {
    sqlx::query_as::<_, Document>("SELECT * FROM documents WHERE id = $1")
        .bind(id)
        .fetch_optional(ex)
        .await
}

pub async fn lister_par_vehicule(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
) -> sqlx::Result<Vec<Document>> {
    sqlx::query_as::<_, Document>(
        "SELECT * FROM documents WHERE vehicule_id = $1 ORDER BY cree_le DESC, id DESC",
    )
    .bind(vehicule_id)
    .fetch_all(ex)
    .await
}

/// Les documents joints aux interventions données (leurs factures, #100), du plus ancien au plus
/// récent. Une requête pour toute une liste.
pub async fn lister_par_interventions(
    ex: impl PgExecutor<'_>,
    intervention_ids: &[i64],
) -> sqlx::Result<Vec<Document>> {
    sqlx::query_as::<_, Document>(
        "SELECT * FROM documents WHERE intervention_id = ANY($1)
         ORDER BY intervention_id, cree_le, id",
    )
    .bind(intervention_ids)
    .fetch_all(ex)
    .await
}

pub async fn lister_par_conducteur(
    ex: impl PgExecutor<'_>,
    conducteur_id: i64,
) -> sqlx::Result<Vec<Document>> {
    sqlx::query_as::<_, Document>(
        "SELECT * FROM documents WHERE conducteur_id = $1 ORDER BY cree_le DESC, id DESC",
    )
    .bind(conducteur_id)
    .fetch_all(ex)
    .await
}

pub async fn inserer(ex: impl PgExecutor<'_>, d: &NouveauDocument) -> sqlx::Result<Document> {
    sqlx::query_as::<_, Document>(
        "INSERT INTO documents
            (vehicule_id, conducteur_id, intervention_id, type_document, nom_original, chemin,
             type_mime, taille_octets, vignette)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING *",
    )
    .bind(d.vehicule_id)
    .bind(d.conducteur_id)
    .bind(d.intervention_id)
    .bind(d.type_document.code())
    .bind(&d.nom_original)
    .bind(&d.chemin)
    .bind(&d.type_mime)
    .bind(d.taille_octets)
    .bind(&d.vignette)
    .fetch_one(ex)
    .await
}

pub async fn supprimer(ex: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let resultat = sqlx::query("DELETE FROM documents WHERE id = $1")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(resultat.rows_affected() == 1)
}

/// Retire le choix de la photo de tête d'un véhicule : la plus récente reprend.
pub async fn retirer_la_photo_de_tete(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
) -> sqlx::Result<u64> {
    let resultat = sqlx::query(
        "UPDATE documents SET photo_de_tete = FALSE WHERE vehicule_id = $1 AND photo_de_tete",
    )
    .bind(vehicule_id)
    .execute(ex)
    .await?;
    Ok(resultat.rows_affected())
}

/// Pose le choix sur une photo. Vrai quand le document est bien une photo (avec sa vignette) du
/// véhicule : la base refuse de toute façon un autre type, mais l'appelant veut un refus net.
pub async fn poser_la_photo_de_tete(
    ex: impl PgExecutor<'_>,
    vehicule_id: i64,
    id: i64,
) -> sqlx::Result<bool> {
    let resultat = sqlx::query(
        "UPDATE documents SET photo_de_tete = TRUE
         WHERE id = $1 AND vehicule_id = $2 AND type_document = 'photo_vehicule'
           AND vignette <> ''",
    )
    .bind(id)
    .bind(vehicule_id)
    .execute(ex)
    .await?;
    Ok(resultat.rows_affected() == 1)
}
