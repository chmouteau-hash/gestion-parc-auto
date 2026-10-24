//! Le frein aux essais de mot de passe (#170).
//!
//! Argon2id rend chaque essai lent ; ce frein rend les essais **rares** : à partir du troisième
//! échec sur un même identifiant, celui-ci est bloqué 30 secondes, puis le double à chaque échec
//! suivant, jusqu'à 15 minutes. Une connexion réussie efface le compteur ; un identifiant sans
//! échec depuis 15 minutes est oublié.
//!
//! Le compteur vit en mémoire du serveur : un redémarrage le remet à zéro, ce qui est acceptable
//! (le frein ralentit, il n'interdit pas). Il porte sur l'identifiant **saisi**, que le compte
//! existe ou non : le message est le même dans les deux cas, pour ne pas révéler la liste des
//! comptes. Pas de blocage par adresse : derrière nginx ou Tailscale, le serveur ne voit que
//! `127.0.0.1`.
//!
//! L'horloge est passée en paramètre (`maintenant`) : les tests déroulent le temps sans attendre.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Nombre d'échecs à partir duquel l'identifiant est bloqué.
pub const SEUIL: u32 = 3;
/// Durée du premier blocage ; elle double à chaque échec suivant.
pub const PREMIER_BLOCAGE: Duration = Duration::from_secs(30);
/// Durée maximale d'un blocage.
pub const BLOCAGE_MAX: Duration = Duration::from_secs(15 * 60);
/// Un identifiant sans échec depuis ce délai est oublié.
pub const OUBLI: Duration = Duration::from_secs(15 * 60);
/// Garde-fou mémoire : au-delà, seuls les identifiants encore bloqués sont gardés.
const ENTREES_MAX: usize = 10_000;

#[derive(Debug, Default)]
pub struct Frein {
    entrees: Mutex<HashMap<String, Entree>>,
}

#[derive(Debug)]
struct Entree {
    echecs: u32,
    dernier_echec: Instant,
    bloque_jusqua: Option<Instant>,
}

/// La clé du compteur : l'identifiant sans ses espaces ni sa casse (« Gestion » et « gestion »
/// sont le même compte à bloquer).
fn cle(login: &str) -> String {
    login.trim().to_lowercase()
}

impl Frein {
    pub fn new() -> Frein {
        Frein::default()
    }

    /// Le temps qui reste avant de pouvoir réessayer, si l'identifiant est bloqué.
    pub fn attente(&self, login: &str, maintenant: Instant) -> Option<Duration> {
        let entrees = self.entrees.lock().unwrap_or_else(|e| e.into_inner());
        entrees
            .get(&cle(login))
            .and_then(|e| e.bloque_jusqua)
            .and_then(|fin| fin.checked_duration_since(maintenant))
            .filter(|reste| !reste.is_zero())
    }

    /// Note un échec. Rend la durée du blocage qui commence, s'il y en a un.
    pub fn echec(&self, login: &str, maintenant: Instant) -> Option<Duration> {
        let mut entrees = self.entrees.lock().unwrap_or_else(|e| e.into_inner());
        purger(&mut entrees, maintenant);
        let entree = entrees.entry(cle(login)).or_insert(Entree {
            echecs: 0,
            dernier_echec: maintenant,
            bloque_jusqua: None,
        });
        entree.echecs += 1;
        entree.dernier_echec = maintenant;
        if entree.echecs < SEUIL {
            return None;
        }
        let duree = duree_du_blocage(entree.echecs - SEUIL);
        entree.bloque_jusqua = Some(maintenant + duree);
        Some(duree)
    }

    /// Une connexion réussie : le compteur de cet identifiant disparaît.
    pub fn reussite(&self, login: &str) {
        let mut entrees = self.entrees.lock().unwrap_or_else(|e| e.into_inner());
        entrees.remove(&cle(login));
    }
}

/// 30 s, 1 min, 2 min, 4 min, 8 min, puis 15 min.
fn duree_du_blocage(echecs_au_dela_du_seuil: u32) -> Duration {
    let facteur = 1u32
        .checked_shl(echecs_au_dela_du_seuil)
        .unwrap_or(u32::MAX);
    PREMIER_BLOCAGE
        .checked_mul(facteur)
        .map_or(BLOCAGE_MAX, |d| d.min(BLOCAGE_MAX))
}

fn purger(entrees: &mut HashMap<String, Entree>, maintenant: Instant) {
    let encore_bloque = |e: &Entree| e.bloque_jusqua.is_some_and(|fin| fin > maintenant);
    entrees.retain(|_, e| encore_bloque(e) || maintenant.duration_since(e.dernier_echec) < OUBLI);
    if entrees.len() > ENTREES_MAX {
        entrees.retain(|_, e| encore_bloque(e));
    }
}

/// « 30 secondes », « 2 minutes » : le temps à dire à qui attend, arrondi au-dessus.
pub fn libelle_attente(duree: Duration) -> String {
    let secondes = duree.as_secs() + u64::from(duree.subsec_nanos() > 0);
    let secondes = secondes.max(1);
    if secondes < 60 {
        format!("{secondes} seconde{}", if secondes > 1 { "s" } else { "" })
    } else {
        let minutes = secondes.div_ceil(60);
        format!("{minutes} minute{}", if minutes > 1 { "s" } else { "" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn deux_echecs_ne_bloquent_pas_le_troisieme_bloque_trente_secondes() {
        let frein = Frein::new();
        let t0 = Instant::now();
        assert_eq!(frein.echec("gestion", t0), None);
        assert_eq!(frein.echec("gestion", t0 + s(1)), None);
        assert_eq!(frein.attente("gestion", t0 + s(2)), None);
        assert_eq!(frein.echec("gestion", t0 + s(2)), Some(s(30)));
        assert_eq!(frein.attente("gestion", t0 + s(12)), Some(s(20)));
        assert_eq!(
            frein.attente("gestion", t0 + s(32)),
            None,
            "le blocage est fini"
        );
    }

    #[test]
    fn la_duree_double_puis_plafonne_a_quinze_minutes() {
        let frein = Frein::new();
        let mut t = Instant::now();
        let mut durees = Vec::new();
        for _ in 0..9 {
            if let Some(d) = frein.echec("gestion", t) {
                durees.push(d.as_secs());
            }
            t += s(10);
        }
        assert_eq!(durees, [30, 60, 120, 240, 480, 900, 900]);
    }

    #[test]
    fn une_reussite_efface_le_compteur() {
        let frein = Frein::new();
        let t0 = Instant::now();
        frein.echec("gestion", t0);
        frein.echec("gestion", t0);
        frein.reussite("gestion");
        assert_eq!(frein.echec("gestion", t0), None, "on repart de zéro");
    }

    #[test]
    fn l_identifiant_est_compare_sans_casse_ni_espaces() {
        let frein = Frein::new();
        let t0 = Instant::now();
        frein.echec("gestion", t0);
        frein.echec(" Gestion ", t0);
        assert_eq!(frein.echec("GESTION", t0), Some(s(30)));
        assert!(frein.attente("gestion", t0).is_some());
        assert_eq!(frein.attente("autre", t0), None);
    }

    #[test]
    fn un_identifiant_sans_echec_depuis_quinze_minutes_est_oublie() {
        let frein = Frein::new();
        let t0 = Instant::now();
        frein.echec("gestion", t0);
        frein.echec("gestion", t0);
        // Un échec d'un autre identifiant, bien plus tard, déclenche la purge.
        frein.echec("autre", t0 + OUBLI + s(1));
        assert_eq!(
            frein.echec("gestion", t0 + OUBLI + s(2)),
            None,
            "compteur oublié"
        );
    }

    #[test]
    fn libelles() {
        assert_eq!(libelle_attente(s(30)), "30 secondes");
        assert_eq!(libelle_attente(Duration::from_millis(1500)), "2 secondes");
        assert_eq!(libelle_attente(Duration::from_millis(200)), "1 seconde");
        assert_eq!(libelle_attente(s(60)), "1 minute");
        assert_eq!(libelle_attente(s(61)), "2 minutes");
        assert_eq!(libelle_attente(s(900)), "15 minutes");
    }
}
