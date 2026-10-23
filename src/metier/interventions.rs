//! Interventions d'entretien : validation et opérations.
//!
//! Une intervention peut détailler **ce qui a été fait** : une ligne par opération (vidange,
//! filtre à huile, bougies…), avec, pour mémoire, la pièce, sa quantité et son prix (#94,
//! bloc 1). Les lignes se saisissent avec l'intervention et se remplacent avec elle : modifier
//! une intervention réécrit toutes ses lignes, dans la même transaction.
//!
//! Elle peut aussi joindre **sa facture** (#100) : un document du véhicule, relié à
//! l'intervention, déposé par le même formulaire. La facture est contrôlée avant d'écrire quoi
//! que ce soit, écrite après : comme la photo d'un véhicule à sa création.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use chrono::NaiveDate;

use crate::commun::format::{date_fr, nombre_fr, quantite_fr};
use crate::commun::types::{Choix, Operation, ResultatControle, TypeDocument, TypeIntervention};
use crate::donnees::documents as donnees_documents;
use crate::donnees::interventions::{self, Intervention, NouvelleIntervention};
use crate::donnees::lignes_intervention::{self, LigneIntervention, NouvelleLigne};
use crate::donnees::vehicules;
use crate::erreurs::{a_le_code, ErreurApp, Resultat, CODE_VIOLATION_CLE_ETRANGERE};
use crate::metier::documents::{self as metier_documents, Rattachement, SaisieDocument};
use crate::metier::echeances::{self as metier_echeances, Solde, SuiteContreVisite};
use crate::metier::validation::{
    centimes_en_saisie, code_parmi, date_obligatoire, entier_positif, identifiant,
    montant_en_centimes, notes, quantite_en_centiemes, texte_facultatif,
};

/// Une opération faite pendant l'intervention, telle que saisie (formulaire ou JSON).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct SaisieLigne {
    /// Code du vocabulaire [`Operation`].
    #[serde(default)]
    pub operation: String,
    /// Ce qu'est l'opération quand elle est « autre ».
    #[serde(default)]
    pub libelle: String,
    #[serde(default)]
    pub reference: String,
    /// Quantité, décimale au besoin (`5,4`) ; vide = 1.
    #[serde(default)]
    pub quantite: String,
    /// Montant en euros (`12,50`) ; vide = 0.
    #[serde(default)]
    pub prix: String,
}

impl SaisieLigne {
    pub fn depuis(l: &LigneIntervention) -> SaisieLigne {
        SaisieLigne {
            operation: l.operation.clone(),
            libelle: l.libelle.clone(),
            reference: l.reference_piece.clone(),
            quantite: quantite_fr(l.quantite_centiemes),
            prix: if l.prix_centimes == 0 {
                String::new()
            } else {
                centimes_en_saisie(l.prix_centimes)
            },
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SaisieIntervention {
    #[serde(default)]
    pub vehicule_id: String,
    #[serde(default)]
    pub type_intervention: String,
    /// Résultat d'un contrôle technique ou d'une contre-visite (#107) ; sans objet sinon.
    #[serde(default)]
    pub resultat_controle: String,
    #[serde(default)]
    pub date_intervention: String,
    #[serde(default)]
    pub kilometrage: String,
    /// Montant en euros (`149,90`), converti en centimes à la validation.
    #[serde(default)]
    pub cout: String,
    #[serde(default)]
    pub prestataire: String,
    #[serde(default)]
    pub notes: String,
    /// Ce qui a été fait. Absent du formulaire « Nouvelle intervention » appelé par l'adresse
    /// (`Query`) : le vocabulaire des adresses ne porte pas de liste.
    #[serde(default)]
    pub lignes: Vec<SaisieLigne>,
}

impl SaisieIntervention {
    pub fn depuis(i: &Intervention) -> SaisieIntervention {
        SaisieIntervention {
            vehicule_id: i.vehicule_id.to_string(),
            type_intervention: i.type_intervention.clone(),
            resultat_controle: i.resultat_controle.clone(),
            date_intervention: i.date_intervention.to_string(),
            kilometrage: i.kilometrage.to_string(),
            cout: centimes_en_saisie(i.cout_centimes),
            prestataire: i.prestataire.clone(),
            notes: i.notes.clone(),
            lignes: i.lignes.iter().map(SaisieLigne::depuis).collect(),
        }
    }

    /// Reconstruit la saisie à partir des paires `nom=valeur` du formulaire HTML. Chaque case
    /// « Ce qui a été fait » cochée envoie `operation=<code>` ; ses précisions voyagent dans
    /// `reference_<code>`, `quantite_<code>`, `prix_<code>`, et `libelle_autre` pour « autre ».
    /// Les champs d'une case non cochée sont envoyés aussi (ils sont dans la page) : ils sont
    /// ignorés. Un formulaire HTML ne sait pas envoyer une liste de structures, d'où ce
    /// décodage à la main ; l'API JSON reçoit `lignes` directement.
    pub fn depuis_formulaire(paires: &[(String, String)]) -> SaisieIntervention {
        let champ = |nom: &str| -> String {
            paires
                .iter()
                .find(|(n, _)| n == nom)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        let lignes = paires
            .iter()
            .filter(|(n, _)| n == "operation")
            .map(|(_, code)| SaisieLigne {
                operation: code.clone(),
                libelle: champ(&format!("libelle_{code}")),
                reference: champ(&format!("reference_{code}")),
                quantite: champ(&format!("quantite_{code}")),
                prix: champ(&format!("prix_{code}")),
            })
            .collect();
        SaisieIntervention {
            vehicule_id: champ("vehicule_id"),
            type_intervention: champ("type_intervention"),
            resultat_controle: champ("resultat_controle"),
            date_intervention: champ("date_intervention"),
            kilometrage: champ("kilometrage"),
            cout: champ("cout"),
            prestataire: champ("prestataire"),
            notes: champ("notes"),
            lignes,
        }
    }

    /// Pour le formulaire : la case de cette opération est-elle cochée ?
    pub fn cochee(&self, code: &str) -> bool {
        self.lignes.iter().any(|l| l.operation == code)
    }

    /// Pour le formulaire : la ligne de cette opération, ou une ligne vide à afficher derrière
    /// une case non cochée.
    pub fn ligne(&self, code: &str) -> SaisieLigne {
        self.lignes
            .iter()
            .find(|l| l.operation == code)
            .cloned()
            .unwrap_or_default()
    }
}

/// Une intervention validée avec ses lignes, prête à écrire.
#[derive(Debug, Clone)]
pub struct InterventionValidee {
    pub intervention: NouvelleIntervention,
    pub lignes: Vec<NouvelleLigne>,
}

/// Un contrôle technique ou une contre-visite : les interventions qui ont un résultat (#107).
pub fn est_un_controle(type_intervention: TypeIntervention) -> bool {
    matches!(
        type_intervention,
        TypeIntervention::ControleTechnique | TypeIntervention::ContreVisite
    )
}

/// Le résultat du contrôle : obligatoire pour un contrôle technique ou une contre-visite, et
/// ignoré pour les autres types, parce que le formulaire envoie le champ quel que soit le type.
pub fn resultat_du_controle(
    type_intervention: TypeIntervention,
    valeur: &str,
) -> Resultat<Option<ResultatControle>> {
    if !est_un_controle(type_intervention) {
        return Ok(None);
    }
    if valeur.trim().is_empty() {
        return Err(ErreurApp::metier(format!(
            "Indiquer le résultat du « {} » : favorable, ou défavorable.",
            type_intervention.libelle()
        )));
    }
    code_parmi(
        valeur,
        "résultat du contrôle",
        ResultatControle::depuis_code,
    )
    .map(Some)
}

pub fn valider(s: &SaisieIntervention) -> Resultat<InterventionValidee> {
    let type_intervention =
        code_parmi(&s.type_intervention, "type", TypeIntervention::depuis_code)?;
    Ok(InterventionValidee {
        intervention: NouvelleIntervention {
            vehicule_id: identifiant(&s.vehicule_id, "véhicule")?,
            type_intervention,
            resultat_controle: resultat_du_controle(type_intervention, &s.resultat_controle)?,
            date_intervention: date_obligatoire(&s.date_intervention, "date")?,
            kilometrage: entier_positif(&s.kilometrage, "kilométrage")?,
            cout_centimes: montant_en_centimes(&s.cout, "coût")?,
            prestataire: texte_facultatif(&s.prestataire, "prestataire")?,
            notes: notes(&s.notes, "notes")?,
        },
        lignes: valider_lignes(&s.lignes)?,
    })
}

/// Les lignes « ce qui a été fait ». Une opération ne s'indique qu'une fois (la quantité dit
/// « quatre pneus »), sauf « autre », que son libellé obligatoire distingue.
pub fn valider_lignes(lignes: &[SaisieLigne]) -> Resultat<Vec<NouvelleLigne>> {
    let mut validees: Vec<NouvelleLigne> = Vec::with_capacity(lignes.len());
    for l in lignes {
        let operation = code_parmi(&l.operation, "opération", Operation::depuis_code)?;
        let nom = operation.libelle();
        let libelle = texte_facultatif(&l.libelle, "opération « Autre »")?;
        if operation == Operation::Autre && libelle.is_empty() {
            return Err(ErreurApp::metier(
                "Préciser de quoi il s'agit pour l'opération « Autre ».",
            ));
        }
        if operation != Operation::Autre && validees.iter().any(|v| v.operation == operation) {
            return Err(ErreurApp::metier(format!(
                "L'opération « {nom} » est indiquée deux fois : la quantité dit combien."
            )));
        }
        let quantite_centiemes = quantite_en_centiemes(&l.quantite, &format!("quantité ({nom})"))?;
        validees.push(NouvelleLigne {
            operation,
            libelle: if operation == Operation::Autre {
                libelle
            } else {
                String::new()
            },
            reference_piece: texte_facultatif(&l.reference, &format!("référence ({nom})"))?,
            quantite_centiemes,
            prix_centimes: montant_en_centimes(&l.prix, &format!("prix ({nom})"))?,
        });
    }
    Ok(validees)
}

/// Règle (a) du bloc 2 de #94 : **le kilométrage croît avec la date**. Rend la phrase qui avertit
/// quand l'intervention saisie contredit une autre du même véhicule : datée d'avant elle avec un
/// kilométrage plus haut, ou d'après elle avec un kilométrage plus bas. Deux interventions du même
/// jour ne se comparent pas. `modifiee` est l'intervention en cours de modification, qui ne se
/// compare pas à elle-même. Ce n'est qu'un avertissement : le formulaire demande confirmation, il
/// ne refuse pas (une erreur ancienne peut être dans l'autre intervention). Fonction pure.
pub fn kilometrage_a_confirmer(
    date: NaiveDate,
    kilometrage: i32,
    autres: &[Intervention],
    modifiee: Option<i64>,
) -> Option<String> {
    let autres = || autres.iter().filter(move |i| Some(i.id) != modifiee);
    let km = |k: i32| format!("{} km", nombre_fr(i64::from(k)));
    // Avant elle, déjà plus haut : la plus récente de celles-là.
    if let Some(i) = autres()
        .filter(|i| i.date_intervention < date && i.kilometrage > kilometrage)
        .max_by_key(|i| (i.date_intervention, i.kilometrage))
    {
        return Some(format!(
            "Le {}, ce véhicule était déjà à {} ; vous avez saisi {} pour le {}. Le kilométrage croît avec la date : vérifiez l'un et l'autre.",
            date_fr(i.date_intervention),
            km(i.kilometrage),
            km(kilometrage),
            date_fr(date)
        ));
    }
    // Après elle, encore plus bas : la plus ancienne de celles-là.
    autres()
        .filter(|i| i.date_intervention > date && i.kilometrage < kilometrage)
        .min_by_key(|i| (i.date_intervention, i.kilometrage))
        .map(|i| {
            format!(
                "Le {}, ce véhicule n'était qu'à {} ; vous avez saisi {} pour le {}. Le kilométrage croît avec la date : vérifiez l'un et l'autre.",
                date_fr(i.date_intervention),
                km(i.kilometrage),
                km(kilometrage),
                date_fr(date)
            )
        })
}

/// [`kilometrage_a_confirmer`] pour une saisie, avec les interventions du véhicule lues en base.
/// Rien à dire d'une saisie invalide : son refus passe d'abord.
pub async fn kilometrage_a_confirmer_pour(
    pool: &PgPool,
    saisie: &SaisieIntervention,
    modifiee: Option<i64>,
) -> Resultat<Option<String>> {
    let Ok(validee) = valider(saisie) else {
        return Ok(None);
    };
    let i = &validee.intervention;
    let autres = interventions::lister_par_vehicule(pool, i.vehicule_id).await?;
    Ok(kilometrage_a_confirmer(
        i.date_intervention,
        i.kilometrage,
        &autres,
        modifiee,
    ))
}

fn traduire(erreur: sqlx::Error) -> ErreurApp {
    if a_le_code(&erreur, CODE_VIOLATION_CLE_ETRANGERE) {
        ErreurApp::metier("Le véhicule choisi n'existe plus.")
    } else {
        erreur.into()
    }
}

/// Attache à chaque intervention ses lignes et ses documents, lus en une requête chacun pour
/// toute la liste.
async fn completer(pool: &PgPool, mut liste: Vec<Intervention>) -> Resultat<Vec<Intervention>> {
    if liste.is_empty() {
        return Ok(liste);
    }
    let ids: Vec<i64> = liste.iter().map(|i| i.id).collect();
    for ligne in lignes_intervention::lister_par_interventions(pool, &ids).await? {
        if let Some(i) = liste.iter_mut().find(|i| i.id == ligne.intervention_id) {
            i.lignes.push(ligne);
        }
    }
    for document in donnees_documents::lister_par_interventions(pool, &ids).await? {
        if let Some(i) = liste
            .iter_mut()
            .find(|i| Some(i.id) == document.intervention_id)
        {
            i.documents.push(document);
        }
    }
    // Une contre-visite se rattache au dernier contrôle défavorable qui la précède : calculé,
    // pas stocké (#107). Rares : une requête chacune.
    for i in liste.iter_mut() {
        if TypeIntervention::depuis_code(&i.type_intervention)
            == Some(TypeIntervention::ContreVisite)
        {
            i.controle_initial = interventions::dernier_controle_defavorable_avant(
                pool,
                i.vehicule_id,
                i.date_intervention,
            )
            .await?;
        }
    }
    Ok(liste)
}

/// La facture jointe à la saisie : le dossier des documents et le fichier reçu.
pub type Facture<'a> = Option<(&'a Path, &'a SaisieDocument)>;

/// Ce que peut être un document joint à une intervention : sa facture, le procès-verbal d'un
/// contrôle technique, ou autre chose. Les autres types (photo du véhicule, carte grise,
/// assurance…) n'ont pas de sens ici (#104).
pub fn types_de_document() -> Vec<Choix> {
    [
        TypeDocument::Facture,
        TypeDocument::ControleTechnique,
        TypeDocument::Autre,
    ]
    .iter()
    .map(|t| Choix {
        code: t.code(),
        libelle: t.libelle(),
    })
    .collect()
}

/// Le type proposé d'office pour le document joint : le procès-verbal quand l'intervention est
/// un contrôle technique ou une contre-visite, la facture sinon.
pub fn type_de_document_par_defaut(type_intervention: &str) -> TypeDocument {
    match TypeIntervention::depuis_code(type_intervention.trim()) {
        Some(t) if est_un_controle(t) => TypeDocument::ControleTechnique,
        _ => TypeDocument::Facture,
    }
}

/// Le type choisi pour le document joint ; vide = celui proposé d'office. Un type qui n'a pas de
/// sens pour une intervention est refusé, même s'il existe pour un document.
pub fn type_de_document(valeur: &str, type_intervention: &str) -> Resultat<TypeDocument> {
    if valeur.trim().is_empty() {
        return Ok(type_de_document_par_defaut(type_intervention));
    }
    let choisi = code_parmi(valeur, "type du document", TypeDocument::depuis_code)?;
    if types_de_document().iter().any(|c| c.code == choisi.code()) {
        Ok(choisi)
    } else {
        Err(ErreurApp::metier(format!(
            "Un document joint à une intervention est une facture, un contrôle technique ou autre chose, pas « {} ».",
            choisi.libelle()
        )))
    }
}

pub async fn lister(pool: &PgPool) -> Resultat<Vec<Intervention>> {
    completer(pool, interventions::lister(pool).await?).await
}

pub async fn lister_par_vehicule(pool: &PgPool, vehicule_id: i64) -> Resultat<Vec<Intervention>> {
    completer(
        pool,
        interventions::lister_par_vehicule(pool, vehicule_id).await?,
    )
    .await
}

pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<Intervention> {
    let intervention = interventions::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)?;
    Ok(completer(pool, vec![intervention])
        .await?
        .pop()
        .expect("une intervention entrée, une sortie"))
}

/// Une intervention enregistrée, et l'échéance qu'elle a soldée s'il y en a une (#94, bloc 3).
#[derive(Debug, Clone)]
pub struct Enregistree {
    pub intervention: Intervention,
    pub solde: Option<Solde>,
    /// Ce qu'un contrôle technique ou une contre-visite dit de la contre-visite (#107).
    pub contre_visite: Option<SuiteContreVisite>,
}

/// Enregistre l'intervention et ses lignes, relève le kilométrage courant du véhicule si celui
/// de l'intervention est plus grand, et solde l'échéance ouverte qu'elle remplit : un contrôle
/// technique favorable, une révision (#94, bloc 3 ; voir [`metier_echeances::solder_dans`]). Tout
/// est dans une même transaction : soit tout réussit, soit rien.
pub async fn creer(pool: &PgPool, saisie: &SaisieIntervention) -> Resultat<Intervention> {
    let aujourd_hui = chrono::Local::now().date_naive();
    Ok(creer_avec_facture(pool, saisie, None, aujourd_hui)
        .await?
        .intervention)
}

/// [`creer`], puis dépose la facture jointe, s'il y en a une, comme document du véhicule relié
/// à l'intervention (#100). La facture est contrôlée (fichier vide, trop gros, type) **avant**
/// d'écrire l'intervention : un refus ne laisse rien derrière lui, et le formulaire revient avec
/// sa saisie. Rend aussi l'échéance soldée et sa suivante, datée de la visite, que le formulaire
/// propose aussitôt.
pub async fn creer_avec_facture(
    pool: &PgPool,
    saisie: &SaisieIntervention,
    facture: Facture<'_>,
    aujourd_hui: NaiveDate,
) -> Resultat<Enregistree> {
    let validee = valider(saisie)?;
    let facture_prete = match facture {
        Some((_, f)) => Some(metier_documents::preparer(f).await?),
        None => None,
    };
    let mut transaction = pool.begin().await?;
    let id = interventions::inserer(&mut *transaction, &validee.intervention)
        .await
        .map_err(traduire)?;
    for ligne in &validee.lignes {
        lignes_intervention::inserer(&mut *transaction, id, ligne).await?;
    }
    vehicules::relever_kilometrage(
        &mut *transaction,
        validee.intervention.vehicule_id,
        validee.intervention.kilometrage,
    )
    .await?;
    let gestes =
        metier_echeances::solder_dans(&mut transaction, &validee.intervention, aujourd_hui).await?;
    transaction.commit().await?;
    joindre(pool, &validee, id, facture, facture_prete).await?;
    let intervention = obtenir(pool, id).await?;
    let solde = match gestes.soldee {
        Some((echeance_id, type_echeance)) => Some(Solde {
            type_echeance,
            echeance: metier_echeances::obtenir(pool, echeance_id).await?,
            suivante: gestes.suivante,
        }),
        None => None,
    };
    Ok(Enregistree {
        intervention,
        solde,
        contre_visite: gestes.contre_visite,
    })
}

/// Écrit la facture contrôlée, reliée à l'intervention, une fois celle-ci enregistrée.
async fn joindre(
    pool: &PgPool,
    validee: &InterventionValidee,
    id: i64,
    facture: Facture<'_>,
    prete: Option<metier_documents::DepotPrepare>,
) -> Resultat<()> {
    if let (Some((documents_dir, _)), Some(prete)) = (facture, prete) {
        metier_documents::ecrire(
            pool,
            documents_dir,
            Rattachement::Vehicule(validee.intervention.vehicule_id),
            prete.pour_intervention(id),
        )
        .await?;
    }
    Ok(())
}

/// Modifie l'intervention et **remplace** ses lignes par celles de la saisie.
pub async fn modifier(
    pool: &PgPool,
    id: i64,
    saisie: &SaisieIntervention,
) -> Resultat<Intervention> {
    modifier_avec_facture(pool, id, saisie, None).await
}

/// [`modifier`], puis dépose la facture jointe, s'il y en a une : la facture arrive parfois
/// après l'intervention (#100). Même ordre qu'à la création : contrôle avant, écriture après.
pub async fn modifier_avec_facture(
    pool: &PgPool,
    id: i64,
    saisie: &SaisieIntervention,
    facture: Facture<'_>,
) -> Resultat<Intervention> {
    let validee = valider(saisie)?;
    let facture_prete = match facture {
        Some((_, f)) => Some(metier_documents::preparer(f).await?),
        None => None,
    };
    let mut transaction = pool.begin().await?;
    if !interventions::mettre_a_jour(&mut *transaction, id, &validee.intervention)
        .await
        .map_err(traduire)?
    {
        return Err(ErreurApp::Introuvable);
    }
    lignes_intervention::supprimer_par_intervention(&mut *transaction, id).await?;
    for ligne in &validee.lignes {
        lignes_intervention::inserer(&mut *transaction, id, ligne).await?;
    }
    vehicules::relever_kilometrage(
        &mut *transaction,
        validee.intervention.vehicule_id,
        validee.intervention.kilometrage,
    )
    .await?;
    transaction.commit().await?;
    joindre(pool, &validee, id, facture, facture_prete).await?;
    obtenir(pool, id).await
}

pub async fn supprimer(pool: &PgPool, id: i64) -> Resultat<()> {
    if interventions::supprimer(pool, id).await? {
        Ok(())
    } else {
        Err(ErreurApp::Introuvable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saisie_valide() -> SaisieIntervention {
        SaisieIntervention {
            vehicule_id: "3".into(),
            type_intervention: "revision".into(),
            date_intervention: "2026-04-02".into(),
            kilometrage: "61000".into(),
            cout: "249,90".into(),
            ..Default::default()
        }
    }

    fn ligne(operation: &str) -> SaisieLigne {
        SaisieLigne {
            operation: operation.into(),
            ..Default::default()
        }
    }

    fn intervention(id: i64, date: (i32, u32, u32), kilometrage: i32) -> Intervention {
        Intervention {
            id,
            vehicule_id: 3,
            type_intervention: "revision".into(),
            resultat_controle: String::new(),
            date_intervention: NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
            kilometrage,
            cout_centimes: 0,
            prestataire: String::new(),
            notes: String::new(),
            cree_le: chrono::Utc::now(),
            immatriculation: String::new(),
            couleur: String::new(),
            controle_initial: None,
            lignes: Vec::new(),
            documents: Vec::new(),
        }
    }

    #[test]
    fn le_kilometrage_croit_avec_la_date() {
        let jour = |a, m, j| NaiveDate::from_ymd_opt(a, m, j).unwrap();
        let autres = vec![
            intervention(1, (2024, 3, 10), 30000),
            intervention(2, (2025, 4, 2), 42000),
        ];
        // Entre les deux, cohérent : rien à dire.
        assert_eq!(
            kilometrage_a_confirmer(jour(2024, 9, 1), 35000, &autres, None),
            None
        );
        // Après 2024 mais plus bas : avertit, en nommant l'intervention d'avant.
        assert_eq!(
            kilometrage_a_confirmer(jour(2024, 9, 1), 28000, &autres, None).unwrap(),
            "Le 10/03/2024, ce véhicule était déjà à 30\u{202F}000 km ; vous avez saisi 28\u{202F}000 km pour le 01/09/2024. Le kilométrage croît avec la date : vérifiez l'un et l'autre."
        );
        // Avant 2025 mais plus haut : avertit, en nommant l'intervention d'après.
        assert!(
            kilometrage_a_confirmer(jour(2024, 9, 1), 45000, &autres, None)
                .unwrap()
                .starts_with("Le 02/04/2025, ce véhicule n'était qu'à 42\u{202F}000 km")
        );
        // Le même jour qu'une autre : pas de comparaison.
        assert_eq!(
            kilometrage_a_confirmer(jour(2025, 4, 2), 41000, &autres, None),
            None
        );
        // En modification, l'intervention ne se compare pas à elle-même.
        // L'intervention 2 (02/04/2025, 42 000 km) passe au 01/05/2025 à 41 000 km : seule la 1
        // compte, et elle est plus basse. Comparée à son ancienne version, elle avertirait.
        assert_eq!(
            kilometrage_a_confirmer(jour(2025, 5, 1), 41000, &autres, Some(2)),
            None
        );
        assert!(kilometrage_a_confirmer(jour(2025, 5, 1), 41000, &autres, None).is_some());
    }

    #[test]
    fn cout_en_euros_devient_centimes() {
        let v = valider(&saisie_valide()).unwrap();
        assert_eq!(v.intervention.cout_centimes, 24990);
        assert_eq!(v.intervention.type_intervention, TypeIntervention::Revision);
        assert_eq!(v.intervention.prestataire, "");
        assert!(v.lignes.is_empty());
    }

    #[test]
    fn le_resultat_est_obligatoire_pour_un_controle_et_ignore_ailleurs() {
        let mut ct = saisie_valide();
        ct.type_intervention = "controle_technique".into();
        let refus = match valider(&ct) {
            Err(ErreurApp::Metier(m)) => m,
            autre => panic!("attendu un refus métier, reçu {autre:?}"),
        };
        assert!(
            refus.contains("résultat du « Contrôle technique »"),
            "{refus}"
        );
        ct.resultat_controle = "defavorable".into();
        assert_eq!(
            valider(&ct).unwrap().intervention.resultat_controle,
            Some(ResultatControle::Defavorable)
        );
        ct.resultat_controle = "bof".into();
        assert!(valider(&ct).is_err());

        let mut cv = saisie_valide();
        cv.type_intervention = "contre_visite".into();
        cv.resultat_controle = "favorable".into();
        assert_eq!(
            valider(&cv).unwrap().intervention.resultat_controle,
            Some(ResultatControle::Favorable)
        );

        // Une révision n'a pas de résultat : le champ envoyé par le formulaire est ignoré.
        let mut revision = saisie_valide();
        revision.resultat_controle = "favorable".into();
        assert_eq!(
            valider(&revision).unwrap().intervention.resultat_controle,
            None
        );
    }

    #[test]
    fn vehicule_non_choisi_refuse() {
        let s = SaisieIntervention {
            type_intervention: "revision".into(),
            date_intervention: "2026-04-02".into(),
            kilometrage: "61000".into(),
            ..Default::default()
        };
        assert!(valider(&s).is_err());
    }

    #[test]
    fn les_lignes_se_valident_avec_leurs_valeurs_par_defaut() {
        let lignes = vec![
            ligne("vidange"),
            SaisieLigne {
                operation: "filtre_huile".into(),
                reference: " W712 ".into(),
                quantite: "2".into(),
                prix: "12,50".into(),
                ..Default::default()
            },
            SaisieLigne {
                operation: "autre".into(),
                libelle: "Joint de culasse".into(),
                ..Default::default()
            },
        ];
        let v = valider_lignes(&lignes).unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].operation, Operation::Vidange);
        assert_eq!((v[0].quantite_centiemes, v[0].prix_centimes), (100, 0));
        assert_eq!(v[1].reference_piece, "W712");
        assert_eq!((v[1].quantite_centiemes, v[1].prix_centimes), (200, 1250));
        assert_eq!(v[2].libelle, "Joint de culasse");
    }

    #[test]
    fn une_ligne_fautive_est_refusee_avec_un_message_lisible() {
        let message = |lignes: &[SaisieLigne]| match valider_lignes(lignes) {
            Err(ErreurApp::Metier(m)) => m,
            autre => panic!("attendu un refus métier, reçu {autre:?}"),
        };
        assert!(message(&[ligne("vidange_moteur")]).contains("vidange_moteur"));
        assert!(message(&[ligne("autre")]).contains("Autre"));
        assert!(message(&[ligne("vidange"), ligne("vidange")]).contains("deux fois"));
        let mut zero = ligne("pneus");
        zero.quantite = "0".into();
        assert!(message(&[zero]).contains("nombre positif"));
        let mut decimale = ligne("vidange");
        decimale.quantite = "5,4".into();
        assert_eq!(
            valider_lignes(&[decimale]).unwrap()[0].quantite_centiemes,
            540
        );
        let mut prix = ligne("pneus");
        prix.prix = "abc".into();
        assert!(message(&[prix]).contains("Pneus"));
        // Deux « autre » distincts passent.
        let mut a = ligne("autre");
        a.libelle = "Joint".into();
        let mut b = ligne("autre");
        b.libelle = "Durite".into();
        assert_eq!(valider_lignes(&[a, b]).unwrap().len(), 2);
    }

    #[test]
    fn le_type_du_document_joint_suit_l_intervention_et_reste_dans_sa_liste() {
        assert_eq!(
            type_de_document_par_defaut("controle_technique"),
            TypeDocument::ControleTechnique
        );
        assert_eq!(
            type_de_document_par_defaut("revision"),
            TypeDocument::Facture
        );
        assert_eq!(
            type_de_document_par_defaut("contre_visite"),
            TypeDocument::ControleTechnique
        );
        assert_eq!(type_de_document_par_defaut(""), TypeDocument::Facture);
        assert_eq!(
            type_de_document("", "controle_technique").unwrap(),
            TypeDocument::ControleTechnique
        );
        assert_eq!(
            type_de_document("autre", "controle_technique").unwrap(),
            TypeDocument::Autre
        );
        let refus = |valeur: &str| match type_de_document(valeur, "revision") {
            Err(ErreurApp::Metier(m)) => m,
            autre => panic!("attendu un refus métier, reçu {autre:?}"),
        };
        assert!(refus("photo_vehicule").contains("Photo du véhicule"));
        assert!(refus("carte_grise").contains("Carte grise"));
        // Le ticket d'une dépense (#126) existe comme type, mais pas sur une intervention.
        assert!(refus("ticket").contains("Ticket de carburant ou de péage"));
        assert_eq!(
            types_de_document()
                .iter()
                .map(|c| c.code)
                .collect::<Vec<_>>(),
            ["facture", "controle_technique", "autre"]
        );
    }

    #[test]
    fn le_libelle_ne_se_garde_que_pour_autre() {
        let mut l = ligne("vidange");
        l.libelle = "sans objet".into();
        assert_eq!(valider_lignes(&[l]).unwrap()[0].libelle, "");
    }

    fn paires(liste: &[(&str, &str)]) -> Vec<(String, String)> {
        liste
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn le_formulaire_se_decode_cases_cochees_seulement() {
        let s = SaisieIntervention::depuis_formulaire(&paires(&[
            ("vehicule_id", "3"),
            ("type_intervention", "revision"),
            ("date_intervention", "2026-04-02"),
            ("kilometrage", "61000"),
            ("cout", "249,90"),
            ("prestataire", ""),
            ("notes", "RAS"),
            ("operation", "vidange"),
            ("reference_vidange", "5W30"),
            ("quantite_vidange", ""),
            ("prix_vidange", "45"),
            // Champs d'une case non cochée : présents dans la page, ignorés.
            ("reference_filtre_air", "oublié"),
            ("operation", "autre"),
            ("libelle_autre", "Joint"),
            ("reference_autre", ""),
            ("quantite_autre", "1"),
            ("prix_autre", ""),
        ]));
        assert_eq!(s.vehicule_id, "3");
        assert_eq!(s.notes, "RAS");
        assert_eq!(s.lignes.len(), 2);
        assert_eq!(s.lignes[0].operation, "vidange");
        assert_eq!(s.lignes[0].reference, "5W30");
        assert_eq!(s.lignes[0].prix, "45");
        assert_eq!(s.lignes[1].operation, "autre");
        assert_eq!(s.lignes[1].libelle, "Joint");
        assert!(s.cochee("vidange") && !s.cochee("filtre_air"));
        assert_eq!(s.ligne("filtre_air"), SaisieLigne::default());
    }

    #[test]
    fn un_formulaire_sans_case_cochee_n_a_pas_de_ligne() {
        let s = SaisieIntervention::depuis_formulaire(&paires(&[
            ("vehicule_id", "3"),
            ("reference_vidange", "5W30"),
        ]));
        assert!(s.lignes.is_empty());
    }
}
