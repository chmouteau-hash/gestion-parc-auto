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
    pub type_document: String,
    /// Nom donné par l'utilisateur, pour l'affichage et le téléchargement.
    pub nom_original: String,
    /// Nom du fichier dans `DOCUMENTS_DIR` (un UUID + extension), jamais le nom d'origine.
    pub chemin: String,
    pub type_mime: String,
    pub taille_octets: i64,
    pub cree_le: DateTime<Utc>,
}

impl Document {
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
        matches!(
            self.type_mime.as_str(),
            "image/jpeg" | "image/png" | "image/webp" | "image/gif"
        )
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
    pub type_document: TypeDocument,
    pub nom_original: String,
    pub chemin: String,
    pub type_mime: String,
    pub taille_octets: i64,
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
            (vehicule_id, conducteur_id, type_document, nom_original, chemin, type_mime,
             taille_octets)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING *",
    )
    .bind(d.vehicule_id)
    .bind(d.conducteur_id)
    .bind(d.type_document.code())
    .bind(&d.nom_original)
    .bind(&d.chemin)
    .bind(&d.type_mime)
    .bind(d.taille_octets)
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
