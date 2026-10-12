//! Garde-fous du catalogue d'interface (`storybook/`). Storybook n'est pas lancé ici : ces tests
//! lisent sa configuration, pour qu'un réglage de confidentialité retiré par mégarde fasse
//! échouer la CI au lieu de passer inaperçu. Ils n'ont pas besoin de la base.

use serde_json::Value;

fn lire(chemin: &str) -> String {
    let complet = format!("{}/storybook/{chemin}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&complet).unwrap_or_else(|e| panic!("{complet} illisible : {e}"))
}

/// Les mots de la commande que lance `npm run storybook`.
fn commande_de_lancement() -> Vec<String> {
    let paquet: Value = serde_json::from_str(&lire("package.json")).expect("package.json valide");
    paquet["scripts"]["storybook"]
        .as_str()
        .expect("le script « storybook » existe")
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// Sans `--host`, Storybook répond sur toutes les cartes réseau du poste. L'application, elle,
/// n'écoute que sur `127.0.0.1` : le catalogue suit la même règle.
#[test]
fn le_catalogue_n_ecoute_que_sur_ce_poste() {
    let commande = commande_de_lancement();
    assert!(
        commande.windows(2).any(|w| w == ["--host", "127.0.0.1"]),
        "`--host 127.0.0.1` manque dans le script « storybook » : {commande:?}"
    );
}

/// Sans cette option, Storybook demande une fois par jour à son éditeur s'il existe une version
/// plus récente, en donnant la sienne.
#[test]
fn le_catalogue_ne_cherche_pas_de_mise_a_jour() {
    let commande = commande_de_lancement();
    assert!(
        commande.iter().any(|mot| mot == "--no-version-updates"),
        "`--no-version-updates` manque dans le script « storybook » : {commande:?}"
    );
}

/// Les statistiques d'usage (télémétrie) se coupent dans la configuration, pas dans le script.
#[test]
fn le_catalogue_n_envoie_pas_de_statistiques_d_usage() {
    assert!(
        lire(".storybook/main.js").contains("disableTelemetry: true"),
        "`disableTelemetry: true` manque dans storybook/.storybook/main.js"
    );
}
