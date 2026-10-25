//! Lecture de la configuration depuis les variables d'environnement.
//!
//! Tout ce qui est secret ou propre à une machine (URL de base, clé de session, chemins, port)
//! vient de l'environnement, jamais du code. Les valeurs attendues sont documentées dans
//! `.env.example`.

use std::path::PathBuf;

use anyhow::{bail, Context};

use crate::commun::types::Profil;

/// Longueur minimale de la clé qui chiffre le cookie de session.
pub const LONGUEUR_MIN_SECRET_SESSION: usize = 64;

#[derive(Debug, Clone)]
pub struct Config {
    /// URL de connexion PostgreSQL (`postgres://utilisateur:mot_de_passe@hote:port/base`).
    pub database_url: String,
    /// Clé qui chiffre et signe le cookie de session.
    pub session_secret: String,
    /// Dossier où sont écrits les fichiers déposés.
    pub documents_dir: PathBuf,
    /// Port d'écoute HTTP.
    pub port: u16,
    /// `true` derrière HTTPS : le cookie de session porte alors l'attribut `Secure`.
    pub cookies_securises: bool,
    /// Vue alertes : nombre de jours d'anticipation des échéances à venir.
    pub alertes_horizon_jours: i64,
    /// Vue alertes : nombre de kilomètres d'anticipation des opérations du plan d'entretien
    /// (#94, bloc 2b).
    pub alertes_horizon_km: i64,
    /// RGPD : durée au-delà de laquelle un conducteur inactif est purgé.
    pub conservation_conducteurs_inactifs_jours: i64,
    /// À qui s'adresse l'installation : une entreprise voit tout, un particulier ne voit ni les
    /// affectations ni le service des conducteurs.
    pub profil: Profil,
    /// Les réglages du rappel par courriel (#176) ; `None` tant que `COURRIEL_SMTP_HOTE` n'est pas
    /// renseigné : le serveur web n'en a pas besoin, seule la sous-commande `envoyer-rappels` s'en
    /// sert.
    pub courriel: Option<Courriel>,
}

/// Comment parler au serveur de courrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Securite {
    /// En clair : un serveur sur la machine même, ou un essai. Jamais sur Internet.
    Aucune,
    /// Connexion en clair puis chiffrée (`STARTTLS`), port 587 le plus souvent.
    StartTls,
    /// Chiffrée d'emblée, port 465 le plus souvent.
    Tls,
}

/// Les réglages du courriel, tout ou rien : dès que `COURRIEL_SMTP_HOTE` est là, l'expéditeur
/// et l'adresse de l'application sont exigés. Les destinataires, eux, sont d'abord les comptes
/// qui ont demandé le rappel dans l'application (#182) ; `COURRIEL_DESTINATAIRES` s'y ajoute.
#[derive(Debug, Clone)]
pub struct Courriel {
    pub hote: String,
    pub port: u16,
    pub securite: Securite,
    /// Identifiant et mot de passe SMTP, ensemble ou pas du tout (un serveur local n'en demande pas).
    pub identifiant: Option<String>,
    pub mot_de_passe: Option<String>,
    /// `Autoclair <parc@exemple.fr>` ou `parc@exemple.fr`.
    pub expediteur: String,
    /// Des adresses en plus de celles des comptes (une boîte partagée, par exemple) ; souvent vide.
    pub destinataires: Vec<String>,
    /// La base des liens du courriel (`https://parc.exemple.fr`), sans barre finale.
    pub adresse_application: String,
}

impl Courriel {
    pub fn depuis_environnement() -> anyhow::Result<Option<Courriel>> {
        let hote = match std::env::var("COURRIEL_SMTP_HOTE") {
            Ok(h) if !h.trim().is_empty() => h.trim().to_string(),
            _ => return Ok(None),
        };
        let securite = match facultatif("COURRIEL_SMTP_SECURITE", "starttls")
            .trim()
            .to_lowercase()
            .as_str()
        {
            "starttls" => Securite::StartTls,
            "tls" => Securite::Tls,
            "aucune" => Securite::Aucune,
            autre => bail!(
                "variable COURRIEL_SMTP_SECURITE : starttls, tls ou aucune attendu, reçu « {autre} »"
            ),
        };
        let port_par_defaut: u16 = if securite == Securite::Tls { 465 } else { 587 };
        let port = nombre("COURRIEL_SMTP_PORT", port_par_defaut)?;
        let non_vide = |nom: &str| std::env::var(nom).ok().filter(|v| !v.trim().is_empty());
        let identifiant = non_vide("COURRIEL_SMTP_IDENTIFIANT");
        let mot_de_passe = non_vide("COURRIEL_SMTP_MOT_DE_PASSE");
        if identifiant.is_some() != mot_de_passe.is_some() {
            bail!("COURRIEL_SMTP_IDENTIFIANT et COURRIEL_SMTP_MOT_DE_PASSE vont ensemble");
        }
        let expediteur = obligatoire("COURRIEL_EXPEDITEUR")?.trim().to_string();
        let destinataires: Vec<String> = facultatif("COURRIEL_DESTINATAIRES", "")
            .split(',')
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(str::to_string)
            .collect();
        let adresse_application = obligatoire("COURRIEL_ADRESSE_APPLICATION")?
            .trim()
            .trim_end_matches('/')
            .to_string();
        Ok(Some(Courriel {
            hote,
            port,
            securite,
            identifiant,
            mot_de_passe,
            expediteur,
            destinataires,
            adresse_application,
        }))
    }
}

impl Config {
    /// Lit et vérifie toutes les variables. Une variable obligatoire absente ou une valeur
    /// invalide arrête le programme au démarrage, avec un message qui nomme la variable :
    /// mieux vaut échouer tout de suite qu'à la première requête.
    pub fn depuis_environnement() -> anyhow::Result<Config> {
        let database_url = obligatoire("DATABASE_URL")?;
        let session_secret = obligatoire("SESSION_SECRET")?;
        if session_secret.chars().count() < LONGUEUR_MIN_SECRET_SESSION {
            bail!(
                "SESSION_SECRET doit faire au moins {LONGUEUR_MIN_SECRET_SESSION} caractères \
                 (voir .env.example pour en générer une)"
            );
        }

        Ok(Config {
            database_url,
            session_secret,
            documents_dir: PathBuf::from(facultatif("DOCUMENTS_DIR", "./documents")),
            port: nombre("PORT", 3000)?,
            cookies_securises: booleen("COOKIES_SECURISES", false)?,
            alertes_horizon_jours: nombre("ALERTES_HORIZON_JOURS", 30)?,
            alertes_horizon_km: nombre("ALERTES_HORIZON_KM", 1000)?,
            conservation_conducteurs_inactifs_jours: nombre(
                "CONSERVATION_CONDUCTEURS_INACTIFS_JOURS",
                365,
            )?,
            profil: profil("PROFIL")?,
            courriel: Courriel::depuis_environnement()?,
        })
    }
}

impl Config {
    /// L'horizon des alertes, en jours et en kilomètres : il décide « à venir » partout.
    pub fn horizon(&self) -> crate::metier::alertes::Horizon {
        crate::metier::alertes::Horizon {
            jours: self.alertes_horizon_jours,
            km: self.alertes_horizon_km,
        }
    }
}

fn obligatoire(nom: &str) -> anyhow::Result<String> {
    let valeur = std::env::var(nom).with_context(|| format!("variable {nom} manquante"))?;
    if valeur.trim().is_empty() {
        bail!("variable {nom} vide");
    }
    Ok(valeur)
}

fn facultatif(nom: &str, defaut: &str) -> String {
    std::env::var(nom)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| defaut.to_string())
}

fn nombre<T: std::str::FromStr>(nom: &str, defaut: T) -> anyhow::Result<T> {
    match std::env::var(nom) {
        Ok(valeur) if !valeur.trim().is_empty() => valeur
            .trim()
            .parse()
            .map_err(|_| anyhow::anyhow!("variable {nom} : nombre attendu, reçu « {valeur} »")),
        _ => Ok(defaut),
    }
}

fn booleen(nom: &str, defaut: bool) -> anyhow::Result<bool> {
    match std::env::var(nom) {
        Ok(valeur) if !valeur.trim().is_empty() => match valeur.trim() {
            "true" | "1" | "oui" => Ok(true),
            "false" | "0" | "non" => Ok(false),
            autre => bail!("variable {nom} : true ou false attendu, reçu « {autre} »"),
        },
        _ => Ok(defaut),
    }
}

/// Sans la variable, l'installation est celle d'une entreprise : c'est le comportement d'avant
/// l'arrivée des profils.
fn profil(nom: &str) -> anyhow::Result<Profil> {
    match std::env::var(nom) {
        Ok(valeur) if !valeur.trim().is_empty() => lire_profil(nom, &valeur),
        _ => Ok(Profil::Entreprise),
    }
}

/// Séparée de la lecture de l'environnement, pour se tester sans y toucher.
fn lire_profil(nom: &str, valeur: &str) -> anyhow::Result<Profil> {
    match Profil::depuis_code(valeur.trim()) {
        Some(profil) => Ok(profil),
        None => bail!("variable {nom} : entreprise ou particulier attendu, reçu « {valeur} »"),
    }
}

/// Lit le fichier `.env` du dossier courant, ou à défaut d'un dossier parent, s'il existe (#184).
///
/// Sans fichier, rien à faire : les variables d'environnement suffisent (service, CI). Une ligne
/// illisible arrête l'application : `dotenvy` s'arrête à cette ligne, et les variables des
/// lignes suivantes prendraient sinon leur valeur par défaut sans rien dire (`DOCUMENTS_DIR`,
/// `PORT`, `COOKIES_SECURISES`…), ou manqueraient avec un message qui égare.
///
/// L'erreur de `dotenvy` cite la valeur fautive, qui peut être un mot de passe : elle n'est
/// jamais affichée. Le fichier est relu pour nommer la ligne (voir [`message_ligne_illisible`]).
pub fn charger_fichier_env() -> anyhow::Result<()> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        Err(erreur) if erreur.not_found() => Ok(()),
        Err(dotenvy::Error::LineParse(..)) => {
            let contenu = trouver_fichier_env()
                .and_then(|fichier| std::fs::read_to_string(fichier).ok())
                .unwrap_or_default();
            bail!(message_ligne_illisible(&contenu))
        }
        Err(autre) => bail!("fichier .env illisible : {autre}"),
    }
}

/// Le `.env` que `dotenvy` lit : celui du dossier courant, sinon du plus proche dossier parent.
fn trouver_fichier_env() -> Option<PathBuf> {
    let mut dossier = std::env::current_dir().ok()?;
    loop {
        let candidat = dossier.join(".env");
        if candidat.is_file() {
            return Some(candidat);
        }
        if !dossier.pop() {
            return None;
        }
    }
}

/// Le message d'un `.env` que `dotenvy` n'a pas su lire, d'après son contenu : la première ligne
/// `NOM=valeur` qui ne se lit pas seule, par son numéro et le nom de sa variable, jamais sa
/// valeur. Une valeur sur plusieurs lignes (rare ici) peut égarer ce repérage : le message dit
/// alors « une ligne », sans plus.
pub fn message_ligne_illisible(contenu: &str) -> String {
    let fautive = contenu.lines().enumerate().find_map(|(index, ligne)| {
        let (nom, _) = ligne
            .trim_start()
            .trim_start_matches("export ")
            .split_once('=')?;
        let nom = nom.trim();
        let est_un_nom =
            !nom.is_empty() && nom.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !est_un_nom {
            return None;
        }
        let lue = dotenvy::from_read_iter(ligne.as_bytes()).next()?;
        lue.is_err().then(|| (index + 1, nom.to_string()))
    });
    let quelle = match fautive {
        Some((numero, nom)) => format!("la ligne {numero} ({nom})"),
        None => "une ligne".to_string(),
    };
    format!(
        "fichier .env : {quelle} n'a pas pu être lue, ni aucune ligne après elle. Une valeur qui \
         contient une espace se met entre guillemets, par exemple \
         COURRIEL_EXPEDITEUR=\"Autoclair <parc@exemple.fr>\" (voir .env.example)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chaque ligne de `.env.example`, décommentée telle quelle, se lit (#184) : copier l'exemple
    /// ne doit jamais produire un `.env` qui s'arrête en route. Les valeurs à espace gardent leur
    /// contenu, guillemets ôtés.
    #[test]
    fn l_exemple_se_lit_une_fois_decommente() {
        let decommente: String = include_str!("../.env.example")
            .lines()
            .map(|ligne| match ligne.strip_prefix("# ") {
                Some(reste)
                    if reste.split_once('=').is_some_and(|(nom, _)| {
                        !nom.is_empty()
                            && nom
                                .chars()
                                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                    }) =>
                {
                    reste
                }
                _ => ligne,
            })
            .map(|ligne| format!("{ligne}\n"))
            .collect();
        let mut lues = std::collections::HashMap::new();
        for element in dotenvy::from_read_iter(decommente.as_bytes()) {
            let (nom, valeur) = element.unwrap_or_else(|e| panic!("{}", texte_sans_ligne(&e)));
            lues.insert(nom, valeur);
        }
        assert_eq!(lues["COURRIEL_EXPEDITEUR"], "Autoclair <moi@gmail.com>");
        assert_eq!(lues["COURRIEL_SMTP_HOTE"], "smtp.gmail.com");
        assert_eq!(lues["TZ"], "Europe/Paris");
        // Toutes les variables nommées dans l'exemple ont été lues, jusqu'à la dernière.
        for nom in decommente
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.split_once('=').map(|(nom, _)| nom))
        {
            assert!(lues.contains_key(nom), "{nom} non lue");
        }
    }

    fn texte_sans_ligne(erreur: &dotenvy::Error) -> String {
        match erreur {
            dotenvy::Error::LineParse(..) => "ligne illisible".to_string(),
            autre => autre.to_string(),
        }
    }

    /// Une ligne illisible est nommée par son numéro et sa variable ; sa valeur n'apparaît
    /// jamais (#184).
    #[test]
    fn une_ligne_illisible_est_nommee_sans_sa_valeur() {
        let fichier = "# Rappels
PORT=3001
COURRIEL_EXPEDITEUR=Autoclair <secret-zz9@exemple.fr>
                       DOCUMENTS_DIR=./documents
";
        // `dotenvy` refuse bien ce fichier, et son erreur porte la valeur : d'où le repérage.
        let erreur = dotenvy::from_read_iter(fichier.as_bytes())
            .find_map(Result::err)
            .expect("le fichier devait être refusé");
        let dotenvy::Error::LineParse(cite, _) = &erreur else {
            panic!("erreur inattendue : {}", texte_sans_ligne(&erreur));
        };
        assert!(
            cite.contains("secret-zz9"),
            "ce que dotenvy cite, qu'on n'affiche pas"
        );
        let message = message_ligne_illisible(fichier);
        assert!(
            message.contains("la ligne 3 (COURRIEL_EXPEDITEUR)"),
            "{message}"
        );
        assert!(message.contains("entre guillemets"), "{message}");
        assert!(!message.contains("  "), "pas d'espace doublée : {message}");
        assert!(!message.contains("secret-zz9"), "{message}");

        let message = message_ligne_illisible(
            "export MOT_DE_PASSE=zz9 cache
",
        );
        assert!(message.contains("la ligne 1 (MOT_DE_PASSE)"), "{message}");
        assert!(!message.contains("zz9"), "{message}");
        assert!(message_ligne_illisible("").contains("une ligne n'a pas"));

        // Entre guillemets, la même valeur se lit, chevrons compris.
        let fichier = "COURRIEL_EXPEDITEUR=\"Autoclair <parc@exemple.fr>\"
";
        let (_, valeur) = dotenvy::from_read_iter(fichier.as_bytes())
            .next()
            .expect("une ligne")
            .expect("lue");
        assert_eq!(valeur, "Autoclair <parc@exemple.fr>");
        assert!(message_ligne_illisible(fichier).contains("une ligne n'a pas"));
    }

    #[test]
    fn profil_reconnu() {
        assert_eq!(
            lire_profil("PROFIL", "entreprise").unwrap(),
            Profil::Entreprise
        );
        assert_eq!(
            lire_profil("PROFIL", " particulier ").unwrap(),
            Profil::Particulier
        );
    }

    #[test]
    fn profil_inconnu_refuse_en_nommant_la_variable() {
        let erreur = lire_profil("PROFIL", "Particulier")
            .unwrap_err()
            .to_string();
        assert!(erreur.contains("PROFIL"), "{erreur}");
        assert!(erreur.contains("« Particulier »"), "{erreur}");
    }
}
