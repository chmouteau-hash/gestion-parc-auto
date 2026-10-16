//! Interventions d'entretien : validation et opérations.
//!
//! Une intervention peut détailler **ce qui a été fait** : une ligne par opération (vidange,
//! filtre à huile, bougies…), avec, pour mémoire, la pièce, sa quantité et son prix (#94,
//! bloc 1). Les lignes se saisissent avec l'intervention et se remplacent avec elle : modifier
//! une intervention réécrit toutes ses lignes, dans la même transaction.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::commun::types::{Operation, TypeIntervention};
use crate::donnees::interventions::{self, Intervention, NouvelleIntervention};
use crate::donnees::lignes_intervention::{self, LigneIntervention, NouvelleLigne};
use crate::donnees::vehicules;
use crate::erreurs::{a_le_code, ErreurApp, Resultat, CODE_VIOLATION_CLE_ETRANGERE};
use crate::metier::validation::{
    centimes_en_saisie, code_parmi, date_obligatoire, entier_positif, identifiant,
    montant_en_centimes, notes, texte_facultatif,
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
    /// Vide = 1.
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
            quantite: l.quantite.to_string(),
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

pub fn valider(s: &SaisieIntervention) -> Resultat<InterventionValidee> {
    Ok(InterventionValidee {
        intervention: NouvelleIntervention {
            vehicule_id: identifiant(&s.vehicule_id, "véhicule")?,
            type_intervention: code_parmi(
                &s.type_intervention,
                "type",
                TypeIntervention::depuis_code,
            )?,
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
        let quantite = if l.quantite.trim().is_empty() {
            1
        } else {
            entier_positif(&l.quantite, &format!("quantité ({nom})"))?
        };
        if quantite < 1 {
            return Err(ErreurApp::metier(format!(
                "La quantité de « {nom} » doit être d'au moins 1."
            )));
        }
        validees.push(NouvelleLigne {
            operation,
            libelle: if operation == Operation::Autre {
                libelle
            } else {
                String::new()
            },
            reference_piece: texte_facultatif(&l.reference, &format!("référence ({nom})"))?,
            quantite,
            prix_centimes: montant_en_centimes(&l.prix, &format!("prix ({nom})"))?,
        });
    }
    Ok(validees)
}

fn traduire(erreur: sqlx::Error) -> ErreurApp {
    if a_le_code(&erreur, CODE_VIOLATION_CLE_ETRANGERE) {
        ErreurApp::metier("Le véhicule choisi n'existe plus.")
    } else {
        erreur.into()
    }
}

/// Attache à chaque intervention ses lignes, lues en une seule requête.
async fn avec_lignes(pool: &PgPool, mut liste: Vec<Intervention>) -> Resultat<Vec<Intervention>> {
    if liste.is_empty() {
        return Ok(liste);
    }
    let ids: Vec<i64> = liste.iter().map(|i| i.id).collect();
    for ligne in lignes_intervention::lister_par_interventions(pool, &ids).await? {
        if let Some(i) = liste.iter_mut().find(|i| i.id == ligne.intervention_id) {
            i.lignes.push(ligne);
        }
    }
    Ok(liste)
}

pub async fn lister(pool: &PgPool) -> Resultat<Vec<Intervention>> {
    avec_lignes(pool, interventions::lister(pool).await?).await
}

pub async fn lister_par_vehicule(pool: &PgPool, vehicule_id: i64) -> Resultat<Vec<Intervention>> {
    avec_lignes(
        pool,
        interventions::lister_par_vehicule(pool, vehicule_id).await?,
    )
    .await
}

pub async fn obtenir(pool: &PgPool, id: i64) -> Resultat<Intervention> {
    let intervention = interventions::obtenir(pool, id)
        .await?
        .ok_or(ErreurApp::Introuvable)?;
    Ok(avec_lignes(pool, vec![intervention])
        .await?
        .pop()
        .expect("une intervention entrée, une sortie"))
}

/// Enregistre l'intervention et ses lignes, et relève le kilométrage courant du véhicule si
/// celui de l'intervention est plus grand. Tout est dans une même transaction : soit tout
/// réussit, soit rien.
pub async fn creer(pool: &PgPool, saisie: &SaisieIntervention) -> Resultat<Intervention> {
    let validee = valider(saisie)?;
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
    transaction.commit().await?;
    obtenir(pool, id).await
}

/// Modifie l'intervention et **remplace** ses lignes par celles de la saisie.
pub async fn modifier(
    pool: &PgPool,
    id: i64,
    saisie: &SaisieIntervention,
) -> Resultat<Intervention> {
    let validee = valider(saisie)?;
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

    #[test]
    fn cout_en_euros_devient_centimes() {
        let v = valider(&saisie_valide()).unwrap();
        assert_eq!(v.intervention.cout_centimes, 24990);
        assert_eq!(v.intervention.type_intervention, TypeIntervention::Revision);
        assert_eq!(v.intervention.prestataire, "");
        assert!(v.lignes.is_empty());
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
        assert_eq!((v[0].quantite, v[0].prix_centimes), (1, 0));
        assert_eq!(v[1].reference_piece, "W712");
        assert_eq!((v[1].quantite, v[1].prix_centimes), (2, 1250));
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
        assert!(message(&[zero]).contains("au moins 1"));
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
