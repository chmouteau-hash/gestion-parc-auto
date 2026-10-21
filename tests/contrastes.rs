//! Contrastes des couples de couleurs que `static/style.css` pose réellement.
//!
//! Les couleurs sont lues dans les jetons de la charte (`design-system/autoclair/code/tokens.css`),
//! pas recopiées : si la charte change une teinte, ce test dit quel couple ne tient plus son
//! seuil. Il se lance seul, sans base : `cargo test --test contrastes`.
//!
//! Le calcul est celui des WCAG 2.x : rapport des luminances relatives, plus 0,05 de chaque côté.
//! Seuils : 4,5:1 pour un texte, 3:1 pour une icône, un cadre ou le contour du clavier.

use std::collections::HashMap;

const JETONS: &str = include_str!("../design-system/autoclair/code/tokens.css");

/// Texte courant et élément d'interface : les deux seuils de la charte.
const TEXTE: f64 = 4.5;
const ELEMENT: f64 = 3.0;

/// (premier plan, fond, seuil, où la feuille de style pose ce couple)
const COUPLES: &[(&str, &str, f64, &str)] = &[
    (
        "encre",
        "blanc",
        TEXTE,
        "texte courant sur carte, tableau, formulaire",
    ),
    ("encre", "fond", TEXTE, "texte courant sur le fond de page"),
    (
        "ardoise-600",
        "blanc",
        TEXTE,
        "texte secondaire : menu, aide, intitulés de fiche",
    ),
    (
        "ardoise-600",
        "fond",
        TEXTE,
        "en-tête de tableau, sous-titre",
    ),
    ("ardoise-600", "desactive", TEXTE, "pastille neutre"),
    (
        "ardoise-600",
        "danger-fond",
        TEXTE,
        "intitulé d'une fiche mobile, sur ligne dépassée",
    ),
    (
        "ardoise-600",
        "succes-fond",
        TEXTE,
        "intitulé d'une fiche mobile, sur ligne en cours",
    ),
    (
        "ardoise-600",
        "attention-fond",
        TEXTE,
        "intitulé d'une fiche mobile, sur ligne à venir",
    ),
    (
        "marine-700",
        "blanc",
        TEXTE,
        "liens, bouton secondaire, lien d'évitement",
    ),
    ("marine-700", "fond", TEXTE, "liens sur le fond de page"),
    ("marine-500", "blanc", TEXTE, "lien au survol"),
    (
        "marine-700",
        "marine-100",
        TEXTE,
        "page en cours du menu, bouton secondaire pressé",
    ),
    (
        "marine-700",
        "marine-50",
        TEXTE,
        "survol du menu et du bouton secondaire",
    ),
    ("marine-700", "cyan-400", TEXTE, "bouton principal"),
    (
        "marine-700",
        "cyan-300",
        TEXTE,
        "bouton principal au survol",
    ),
    ("marine-700", "cyan-500", TEXTE, "bouton principal pressé"),
    ("succes", "succes-fond", TEXTE, "pastille « à jour »"),
    ("succes", "blanc", TEXTE, "même pastille, sur ligne teintée"),
    ("attention", "attention-fond", TEXTE, "pastille « bientôt »"),
    (
        "marine-700",
        "cyan-100",
        TEXTE,
        "chiffre « en service » du tableau de bord",
    ),
    (
        "ardoise-600",
        "attention-fond",
        TEXTE,
        "délai dans un bandeau « à venir »",
    ),
    (
        "attention",
        "blanc",
        TEXTE,
        "même pastille, sur ligne teintée",
    ),
    (
        "danger",
        "danger-fond",
        TEXTE,
        "pastille « dépassé », bouton danger au survol",
    ),
    (
        "danger",
        "blanc",
        TEXTE,
        "même pastille, sur ligne teintée ; bouton danger",
    ),
    (
        "encre",
        "danger-fond",
        TEXTE,
        "texte d'un message d'erreur, d'une ligne dépassée",
    ),
    (
        "encre",
        "succes-fond",
        TEXTE,
        "texte d'un message de réussite, d'une ligne en cours",
    ),
    (
        "encre",
        "attention-fond",
        TEXTE,
        "texte d'une ligne à venir",
    ),
    (
        "blanc",
        "marine-900",
        TEXTE,
        "barre latérale Pro : menu, compte, en haut du dégradé",
    ),
    (
        "blanc",
        "marine-700",
        TEXTE,
        "barre latérale Pro : menu, compte, en bas du dégradé",
    ),
    (
        "blanc",
        "marine-500",
        TEXTE,
        "entrée de la barre latérale au survol",
    ),
    (
        "marine-900",
        "cyan-400",
        TEXTE,
        "page en cours dans la barre latérale Pro",
    ),
    (
        "marine-700",
        "marine-100",
        TEXTE,
        "initiales en rond d'un conducteur",
    ),
    ("ardoise-500", "blanc", ELEMENT, "bord d'un champ de saisie"),
    (
        "danger",
        "blanc",
        ELEMENT,
        "cadre et icône d'un message d'erreur",
    ),
    (
        "succes",
        "blanc",
        ELEMENT,
        "cadre et icône d'un message de réussite",
    ),
    ("focus", "blanc", ELEMENT, "contour du clavier"),
    (
        "focus",
        "fond",
        ELEMENT,
        "contour du clavier sur le fond de page",
    ),
    // Couleurs de repère d'un véhicule : la bande de la plaque (sur blanc) et la silhouette (sur
    // marine-50). Des surfaces décoratives, le texte dit toujours le véhicule ; on leur demande
    // quand même le seuil d'un élément, pour qu'elles se voient. Le blanc et le jaune ne
    // peuvent pas le tenir : l'échantillon est cerclé pour eux, et la plaque garde son contour.
    // Le beige et les niveaux clairs du gris, du vert et du bleu sont dans leur cas (voir plus bas).
    (
        "repere-gris",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère gris",
    ),
    (
        "repere-noir",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère noir",
    ),
    (
        "repere-rouge",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère rouge",
    ),
    (
        "repere-orange",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère orange",
    ),
    (
        "repere-vert",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère vert",
    ),
    (
        "repere-bleu",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère bleu",
    ),
    (
        "repere-violet",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère violet",
    ),
    (
        "repere-marron",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère marron",
    ),
    (
        "repere-gris",
        "marine-50",
        ELEMENT,
        "silhouette, repère gris",
    ),
    (
        "repere-noir",
        "marine-50",
        ELEMENT,
        "silhouette, repère noir",
    ),
    (
        "repere-rouge",
        "marine-50",
        ELEMENT,
        "silhouette, repère rouge",
    ),
    (
        "repere-orange",
        "marine-50",
        ELEMENT,
        "silhouette, repère orange",
    ),
    (
        "repere-vert",
        "marine-50",
        ELEMENT,
        "silhouette, repère vert",
    ),
    (
        "repere-bleu",
        "marine-50",
        ELEMENT,
        "silhouette, repère bleu",
    ),
    (
        "repere-violet",
        "marine-50",
        ELEMENT,
        "silhouette, repère violet",
    ),
    (
        "repere-marron",
        "marine-50",
        ELEMENT,
        "silhouette, repère marron",
    ),
    // Les niveaux foncés du gris, du rouge, du vert et du bleu, le rouge clair (un rouge vif, pas
    // un rose) et le marron clair tiennent le seuil sur les deux fonds.
    (
        "repere-rouge-clair",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère rouge clair",
    ),
    (
        "repere-rouge-clair",
        "marine-50",
        ELEMENT,
        "silhouette, repère rouge clair",
    ),
    (
        "repere-marron-clair",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère marron clair",
    ),
    (
        "repere-marron-clair",
        "marine-50",
        ELEMENT,
        "silhouette, repère marron clair",
    ),
    (
        "repere-gris-fonce",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère gris foncé",
    ),
    (
        "repere-rouge-fonce",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère rouge foncé",
    ),
    (
        "repere-vert-fonce",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère vert foncé",
    ),
    (
        "repere-bleu-fonce",
        "blanc",
        ELEMENT,
        "bande de la plaque, repère bleu foncé",
    ),
    (
        "repere-gris-fonce",
        "marine-50",
        ELEMENT,
        "silhouette, repère gris foncé",
    ),
    (
        "repere-rouge-fonce",
        "marine-50",
        ELEMENT,
        "silhouette, repère rouge foncé",
    ),
    (
        "repere-vert-fonce",
        "marine-50",
        ELEMENT,
        "silhouette, repère vert foncé",
    ),
    (
        "repere-bleu-fonce",
        "marine-50",
        ELEMENT,
        "silhouette, repère bleu foncé",
    ),
    // Le beige et les niveaux clairs du gris, du vert et du bleu ne tiennent le seuil ni sur blanc
    // ni sur marine-50 (comme le blanc et le jaune) : l'échantillon est cerclé, la plaque garde son
    // contour, et la silhouette est posée sur un cadre ardoise-600 (`.carte-photo:has(…)`,
    // `static/style.css`), où ils le tiennent.
    (
        "repere-beige",
        "ardoise-600",
        ELEMENT,
        "silhouette, repère beige, sur son cadre sombre",
    ),
    (
        "repere-gris-clair",
        "ardoise-600",
        ELEMENT,
        "silhouette, repère gris clair, sur son cadre sombre",
    ),
    (
        "repere-vert-clair",
        "ardoise-600",
        ELEMENT,
        "silhouette, repère vert clair, sur son cadre sombre",
    ),
    (
        "repere-bleu-clair",
        "ardoise-600",
        ELEMENT,
        "silhouette, repère bleu clair, sur son cadre sombre",
    ),
];

/// Les jetons de couleur du fichier : `--nom: #rrggbb;`, rangés par nom sans les deux tirets.
fn couleurs() -> HashMap<String, [f64; 3]> {
    let mut jetons = HashMap::new();
    for ligne in JETONS.lines() {
        let Some((nom, valeur)) = ligne.trim().split_once(':') else {
            continue;
        };
        let valeur = valeur.trim().trim_end_matches(';');
        let Some(hexa) = valeur.strip_prefix('#').filter(|h| h.len() == 6) else {
            continue;
        };
        let Some(nom) = nom.strip_prefix("--") else {
            continue;
        };
        let canal = |i: usize| u8::from_str_radix(&hexa[i..i + 2], 16).map(f64::from);
        if let (Ok(r), Ok(v), Ok(b)) = (canal(0), canal(2), canal(4)) {
            jetons.insert(nom.to_string(), [r / 255.0, v / 255.0, b / 255.0]);
        }
    }
    jetons
}

fn luminance([r, v, b]: [f64; 3]) -> f64 {
    let lineaire = |c: f64| {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lineaire(r) + 0.7152 * lineaire(v) + 0.0722 * lineaire(b)
}

fn contraste(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (clair, sombre) = {
        let (la, lb) = (luminance(a), luminance(b));
        if la >= lb {
            (la, lb)
        } else {
            (lb, la)
        }
    };
    (clair + 0.05) / (sombre + 0.05)
}

#[test]
fn chaque_couple_de_la_feuille_de_style_tient_son_seuil() {
    let couleurs = couleurs();
    let mut ecarts = Vec::new();
    for (premier, fond, seuil, ou) in COUPLES {
        let (Some(a), Some(b)) = (couleurs.get(*premier), couleurs.get(*fond)) else {
            ecarts.push(format!("jeton absent de tokens.css : {premier} ou {fond}"));
            continue;
        };
        let rapport = contraste(*a, *b);
        if rapport < *seuil {
            ecarts.push(format!(
                "{premier} sur {fond} : {rapport:.2}:1, il faut {seuil}:1 ({ou})"
            ));
        }
    }
    assert!(ecarts.is_empty(), "{}", ecarts.join("\n"));
}

/// Garde-fou du calcul lui-même : le noir sur le blanc fait 21:1, valeur de référence des WCAG.
#[test]
fn le_calcul_donne_la_valeur_de_reference() {
    let noir = [0.0, 0.0, 0.0];
    let blanc = [1.0, 1.0, 1.0];
    assert!((contraste(noir, blanc) - 21.0).abs() < 1e-9);
    assert!((contraste(blanc, blanc) - 1.0).abs() < 1e-9);
}

/// Les jetons de couleur de la charte sont bien tous lus : un format qui changerait dans
/// `tokens.css` ferait lire une liste vide, et le test ci-dessus passerait sur rien.
#[test]
fn les_jetons_de_couleur_sont_lus() {
    let couleurs = couleurs();
    assert!(couleurs.len() >= 20, "{} jetons lus", couleurs.len());
    assert!(couleurs.contains_key("marine-700"));
    assert!(couleurs.contains_key("cyan-400"));
}
