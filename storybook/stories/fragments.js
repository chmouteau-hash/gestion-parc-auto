// Morceaux de page partagés par les stories.
//
// Storybook ne sait pas exécuter les gabarits Askama : chaque fonction ci-dessous recopie le HTML
// d'un gabarit de `templates/`, nommé dans son commentaire. Quand un gabarit change, sa copie est
// à retoucher ici. Les données sont fictives.

// Logos de la charte : Vite en donne l'adresse, comme le serveur les sert sous `/static/logos/`.
import logoAutoclair from "../../design-system/autoclair/logos/svg/autoclair-principale.svg";
import logoAutoclairNegatif from "../../design-system/autoclair/logos/svg/autoclair-principale-negatif.svg";
import logoAutoclairPro from "../../design-system/autoclair/logos/svg/autoclair-pro-principale.svg";
import logoAutoclairProNegatif from "../../design-system/autoclair/logos/svg/autoclair-pro-principale-negatif.svg";

// Vocabulaires de `src/commun/types.rs` : [code, libellé].
export const STATUTS_VEHICULE = [
  ["en_service", "En service"],
  ["en_maintenance", "En maintenance"],
  ["reforme", "Réformé"],
];
export const STATUTS_CONDUCTEUR = [
  ["actif", "Actif"],
  ["inactif", "Inactif"],
];
export const CARBURANTS = [
  ["essence", "Essence"],
  ["diesel", "Diesel"],
  ["electrique", "Électrique"],
  ["hybride", "Hybride"],
  ["gpl", "GPL"],
  ["autre", "Autre"],
];
export const TYPES_INTERVENTION = [
  ["revision", "Révision"],
  ["reparation", "Réparation"],
  ["pneumatiques", "Pneumatiques"],
  ["controle_technique", "Contrôle technique"],
  ["carrosserie", "Carrosserie"],
  ["autre", "Autre"],
];
export const TYPES_DOCUMENT = [
  ["photo_vehicule", "Photo du véhicule"],
  ["facture", "Facture"],
  ["controle_technique", "Contrôle technique"],
  ["carte_grise", "Carte grise"],
  ["assurance", "Assurance"],
  ["permis", "Permis de conduire"],
  ["autre", "Autre"],
];
// États d'une affectation (`src/donnees/affectations.rs`) : [classe de la pastille, libellé].
export const ETATS_AFFECTATION = [
  ["etat-en_cours", "En cours"],
  ["etat-a_venir", "À venir"],
  ["etat-terminee", "Terminée"],
];
// États d'une échéance (`EtatEcheance`, `src/metier/echeances.rs`) : [classe de la pastille, libellé].
export const ETATS_ECHEANCE = [
  ["echeance-depassee", "Dépassée"],
  ["echeance-a_venir", "À venir"],
  ["echeance-a_traiter", "À traiter"],
  ["echeance-traitee", "Traitée"],
];

export const VEHICULES = [
  { id: 1, immatriculation: "AB-123-CD", marque: "Renault", modele: "Clio", miseEnService: "15/03/2022", carburant: "essence", kilometrage: 48200, statut: "en_service" },
  { id: 2, immatriculation: "EF-456-GH", marque: "Peugeot", modele: "Partner", miseEnService: "02/09/2020", carburant: "diesel", kilometrage: 112450, statut: "en_maintenance" },
  { id: 3, immatriculation: "JK-789-LM", marque: "Renault", modele: "Zoe", miseEnService: "20/06/2024", carburant: "electrique", kilometrage: 15800, statut: "en_service" },
  { id: 4, immatriculation: "NP-012-QR", marque: "Citroën", modele: "Berlingo", miseEnService: "11/01/2016", carburant: "diesel", kilometrage: 231000, statut: "reforme" },
];

export const DOCUMENTS = [
  { id: 1, type: "facture", nom: "facture-revision-2026.pdf", taille: "184 Ko", creeLe: "02/10/2026 09:14" },
  { id: 2, type: "controle_technique", nom: "pv-controle-technique.jpg", taille: "2,4 Mo", creeLe: "18/09/2026 16:40" },
  { id: 3, type: "carte_grise", nom: "carte-grise.pdf", taille: "96 Ko", creeLe: "15/03/2022 11:02" },
];

/** Une « photo du véhicule » fictive, dessinée : un rectangle teinté avec une silhouette. Tient lieu de vignette. */
export const PHOTO_FICTIVE =
  "data:image/svg+xml," +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 480"><rect width="640" height="480" fill="#b8c4d6"/><rect y="320" width="640" height="160" fill="#8d9bb0"/><path d="M120 330h400l-40-90H180z" fill="#2d4f86"/><circle cx="200" cy="340" r="34" fill="#1b2a44"/><circle cx="440" cy="340" r="34" fill="#1b2a44"/></svg>`,
  );

/** La vignette d'une carte d'accueil ou d'une fiche : la photo, ou la silhouette (`templates/accueil/particulier.html`). */
export function cartePhoto(vehiculeId, avecPhoto) {
  return `<a class="carte-photo" href="/vehicules/${vehiculeId}" tabindex="-1" aria-hidden="true">${avecPhoto ? `<img src="${PHOTO_FICTIVE}" alt="">` : `<span class="silhouette"></span>`}</a>`;
}

/** Libellé d'un code dans un vocabulaire ; le code lui-même s'il est inconnu, comme dans l'application. */
export function libelle(vocabulaire, code) {
  const trouve = vocabulaire.find(([c]) => c === code);
  return trouve ? trouve[1] : code;
}

/** Options d'une liste déroulante, celle de `selection` étant choisie. */
export function options(vocabulaire, selection = "") {
  return vocabulaire
    .map(([code, texte]) => `<option value="${code}"${code === selection ? " selected" : ""}>${texte}</option>`)
    .join("\n      ");
}

/** `Vehicule::designation` : « AB-123-CD (Renault Clio) ». */
export function designation(vehicule) {
  return `${vehicule.immatriculation} (${vehicule.marque} ${vehicule.modele})`;
}

export function pastille(texte, classe = "") {
  return `<span class="pastille${classe ? ` ${classe}` : ""}">${texte}</span>`;
}

export function pastilleStatut(vocabulaire, code) {
  return pastille(libelle(vocabulaire, code), `statut-${code}`);
}

// Le menu complet d'Autoclair Pro : [code de page, adresse, icône (data-ico), intitulé].
const MENU_PRO = [
  ["accueil", "/", "tableau", "Tableau de bord"],
  ["alertes", "/alertes", "cloche", "Alertes"],
  ["vehicules", "/vehicules", "voiture", "Véhicules"],
  ["conducteurs", "/conducteurs", "personnes", "Conducteurs"],
  ["affectations", "/affectations", "echange", "Affectations"],
  ["interventions", "/interventions", "cle", "Entretien"],
  ["echeances", "/echeances", "calendrier", "Échéances"],
];
// Les quatre onglets d'Autoclair : [onglet, adresse, icône, intitulé].
const ONGLETS = [
  ["vehicules", "/", "voiture", "Véhicules"],
  ["echeances", "/echeances", "calendrier", "Échéances"],
  ["interventions", "/interventions", "cle", "Entretien"],
  ["plus", "/plus", "plus", "Plus"],
];

/** `Contexte::montre_affectations`, `montre_service` et `est_entreprise` (`src/web/rendu.rs`). */
export function estEntreprise(profil) {
  return profil !== "particulier";
}

/** `Contexte::onglet` : l'onglet Autoclair que la page en cours allume. */
export function onglet(page) {
  if (page === "accueil" || page === "vehicules") return "vehicules";
  if (page === "echeances" || page === "interventions") return page;
  return "plus";
}

/**
 * `Profil::marque`, `Profil::logo` et `Profil::logo_negatif` (`src/web/rendu.rs`) : le
 * particulier est « Autoclair », l'entreprise « Autoclair Pro ». Les logos sont les fichiers de
 * la charte, que le serveur sert sous `/static/logos/` ; Storybook les prend au même endroit,
 * dans `design-system/`.
 */
export function marque(profil) {
  return estEntreprise(profil)
    ? { nom: "Autoclair Pro", logo: logoAutoclairPro, logoNegatif: logoAutoclairProNegatif }
    : { nom: "Autoclair", logo: logoAutoclair, logoNegatif: logoAutoclairNegatif };
}

/** Le formulaire de recherche (`templates/base.html`, `plus.html`, `recherche.html`). */
export function recherche({ surLaPage = false, question = "", raccourci = false } = {}) {
  return `<form class="recherche${surLaPage ? " recherche-page" : ""}" method="get" action="/recherche" role="search">
  <input type="search" name="q"${question ? ` value="${question}"` : ""} placeholder="${surLaPage ? "Immatriculation, marque, conducteur…" : "Immatriculation, conducteur…"}" aria-label="Rechercher${raccourci ? " (raccourci : /)" : ""}" autocomplete="off"${raccourci ? " data-raccourci" : ""}>
  <button type="submit" aria-label="Rechercher"></button>
</form>`;
}

function session({ login, admin }) {
  return `<form class="session" method="post" action="/deconnexion">
    <span>${login} · ${admin ? "administrateur" : "lecture seule"}</span>
    <button type="submit" class="bouton-discret">Se déconnecter</button>
  </form>`;
}

/**
 * `templates/base.html`, de `<header>` à `</header>` : la barre latérale d'Autoclair Pro (menu
 * complet, recherche, compte), ou les quatre onglets d'Autoclair.
 */
export function enTete({ page = "alertes", admin = true, login = "gestionnaire", profil = "entreprise" } = {}) {
  const { nom, logo, logoNegatif } = marque(profil);
  const lien = (actif) => ([, adresse, icone, intitule]) =>
    `<a href="${adresse}" data-ico="${icone}"${actif ? ' class="actif" aria-current="page"' : ""}>${intitule}</a>`;
  if (estEntreprise(profil)) {
    const liens = MENU_PRO.map((entree) => lien(entree[0] === page)(entree));
    if (admin) {
      liens.push(lien(page === "documents")(["documents", "/documents/nouveau", "depot", "Déposer un document"]));
    }
    return `<header class="entete laterale">
  <div class="laterale-haut">
    <a class="marque" href="/"><img class="logo" src="${logoNegatif}" alt="${nom}"></a>
    <button type="button" class="menu-bascule" data-menu aria-controls="menu" aria-expanded="false">Menu</button>
  </div>
  <div class="laterale-corps" id="menu">
    ${recherche({ raccourci: true })}
    <nav class="menu">
      ${liens.join("\n      ")}
    </nav>
    ${session({ login, admin })}
  </div>
</header>`;
  }
  const actif = onglet(page);
  return `<header class="entete">
  <a class="marque" href="/"><img class="logo" src="${logo}" alt="${nom}"></a>
  <nav class="menu onglets">
    ${ONGLETS.map((entree) => lien(entree[0] === actif)(entree)).join("\n    ")}
  </nav>
  ${session({ login, admin })}
</header>`;
}

/**
 * `templates/base.html` en entier : le lien d'évitement, l'en-tête, puis le contenu de la page,
 * précédé du nom de l'application, que seule l'impression affiche. La classe du profil (`pro`,
 * `part`), que l'application pose sur `<body>`, est ici sur un bloc qui l'enveloppe.
 */
export function page({ contenu, ...contexte }) {
  return `<div class="${estEntreprise(contexte.profil) ? "pro" : "part"}">
<a class="bouton lien-evitement" href="#contenu">Aller au contenu</a>
${enTete(contexte)}
<main class="contenu" id="contenu">
<p class="pour-impression">${marque(contexte.profil).nom}</p>
${contenu}
</main>
</div>`;
}

/** Le bouton « Imprimer » des fiches, des listes et de la page d'un document (`data-imprimer`, pris en charge par `app.js`). */
export function boutonImprimer(principal = false) {
  return `<button type="button" class="${principal ? "bouton" : "bouton-discret"}" data-imprimer>Imprimer</button>`;
}

/**
 * Les macros de `templates/macros.html`, recopiées. Le A-route de l'écran vide reprend les tracés
 * de `autoclair-a-couleur.svg` ; ici, une silhouette simplifiée suffit à l'aperçu (la feuille de
 * style colore `.a-route-jambes` et `.a-route-sommet` de la même façon).
 */
export function plaque(immatriculation) {
  return `<span class="plaque">${immatriculation}</span>`;
}

export function trajet(classe, dizaine) {
  return `<span class="trajet ${classe} trajet-${dizaine}" aria-hidden="true"><i></i><u><b></b></u><s></s></span>`;
}

export function borne(classe, valeur, unite) {
  return `<div class="borne ${classe}" aria-hidden="true"><b></b><strong>${valeur}</strong><small>${unite}</small></div>`;
}

export function ecranVide(texte, { lien = "", action = "" } = {}) {
  return `<div class="ecran-vide">
  <svg class="a-route" aria-hidden="true" viewBox="0 0 270 200" width="135" height="100"><path class="a-route-jambes" d="M103 4 1 188a8 8 0 0 0 7 12h42c15 0 28-9 34-24l50-124 50 124c6 15 19 24 34 24h42a8 8 0 0 0 7-12L167 4a8 8 0 0 0-7-4H110a8 8 0 0 0-7 4z"/><path class="a-route-sommet" d="M103 4a8 8 0 0 1 7-4h50a8 8 0 0 1 7 4l-29 48h-6z"/></svg>
  <p class="vide">${texte}</p>
  ${action ? `<a class="bouton" href="${lien}">${action}</a>` : ""}
</div>`;
}

/** Page sans menu : `templates/connexion.html` et `templates/erreur.html`. */
export function pageEtroite(contenu) {
  return `<main class="contenu contenu-etroit">
${contenu}
</main>`;
}

/** Cellule de tableau alignée à droite (kilométrage, coût). */
export function nombre(html) {
  return { classe: "nombre", html };
}

/**
 * Tableau de liste. Une ligne est la suite de ses cellules, ou `{ classe, cellules }` pour une
 * ligne colorée. Une cellule est du HTML, ou `nombre("…")`. Un en-tête vide est celui de la
 * colonne des actions : il n'a pas de texte, seulement un nom pour les lecteurs d'écran.
 */
export function tableau(entetes, lignes) {
  const entete = (e) => (e ? `<th>${e}</th>` : `<th aria-label="Actions"></th>`);
  const cellule = (c) => (typeof c === "string" ? `<td>${c}</td>` : `<td class="${c.classe}">${c.html}</td>`);
  const ligne = (l) => {
    const { classe = "", cellules } = Array.isArray(l) ? { cellules: l } : l;
    return `    <tr${classe ? ` class="${classe}"` : ""}>${cellules.map(cellule).join("")}</tr>`;
  };
  return `<table class="tableau">
  <thead>
    <tr>${entetes.map(entete).join("")}</tr>
  </thead>
  <tbody>
${lignes.map(ligne).join("\n")}
  </tbody>
</table>`;
}

/** Bouton d'une ligne de tableau qui agit tout de suite (clôturer, supprimer), après confirmation s'il y en a une. */
export function boutonEnLigne({ action, texte, danger = false, confirmer = "" }) {
  return `<form method="post" action="${action}" class="en-ligne"${confirmer ? ` data-confirmer="${confirmer}"` : ""}><button type="submit" class="bouton-discret${danger ? " danger" : ""}">${texte}</button></form>`;
}

/** `templates/documents/liste.html`. */
export function listeDocuments({ documents = DOCUMENTS, admin = true } = {}) {
  if (documents.length === 0) {
    return `<p class="vide">Aucun document.</p>`;
  }
  const supprimer = (d) =>
    boutonEnLigne({
      action: `/documents/${d.id}/supprimer`,
      texte: "Supprimer",
      danger: true,
      confirmer: `Supprimer le document « ${d.nom} » ?`,
    });
  return tableau(
    ["Type", "Nom", "Taille", "Déposé le", ""],
    documents.map((d) => [
      libelle(TYPES_DOCUMENT, d.type),
      `<a href="/documents/${d.id}">${d.nom}</a>`,
      d.taille,
      d.creeLe,
      `<a href="/documents/${d.id}/telecharger">Télécharger</a>${admin ? ` ${supprimer(d)}` : ""}`,
    ]),
  );
}

// `metier::documents::TAILLE_MAX_OCTETS` et `message_trop_volumineux()`.
const TAILLE_MAX = 'data-taille-max="10485760" data-trop-gros="Le fichier dépasse la taille maximale de 10 Mo."';

/**
 * `templates/documents/depot.html`. Sans `vehicules` (sur la fiche d'un véhicule ou d'un
 * conducteur), la liste des véhicules disparaît. `envoiEnCours` montre l'état que pose `app.js`
 * pendant l'envoi.
 */
export function depot({ action = "/documents", vehicules = [], vehiculeChoisi = "", typeChoisi = "", envoiEnCours = false } = {}) {
  const choixVehicule =
    vehicules.length === 0
      ? ""
      : `
  <label>Véhicule
    <select name="vehicule_id" required>
      <option value="">— choisir —</option>
      ${options(vehicules.map((v) => [String(v.id), designation(v)]), vehiculeChoisi)}
    </select>
  </label>`;
  return `<form method="post" action="${action}" enctype="multipart/form-data" class="formulaire depot${envoiEnCours ? " envoi-en-cours" : ""}" data-envoi-auto>${choixVehicule}
  <label>Type de document
    <select name="type_document" required>
      <option value="">— choisir —</option>
      ${options(TYPES_DOCUMENT, typeChoisi)}
    </select>
  </label>
  <div class="depot-boutons">
    <label class="bouton depot-photo">Prendre une photo
      <input type="file" name="fichier" accept="image/*" capture="environment" ${TAILLE_MAX}>
    </label>
    <label class="bouton-discret">Choisir un fichier
      <input type="file" name="fichier" ${TAILLE_MAX}>
    </label>
    <noscript><button type="submit" class="bouton">Déposer</button></noscript>
  </div>
  <p class="aide" data-etat-envoi>${envoiEnCours ? "Envoi en cours…" : "10 Mo maximum. Le document est enregistré dès que le fichier est choisi."}</p>
</form>`;
}

/** Réglage « largeur de téléphone » d'une story : la barre d'outils reste sur cette largeur. */
export const SUR_TELEPHONE = { viewport: { value: "telephone", isRotated: false } };

/** Réglage commun des stories d'écran : rôle de l'utilisateur connecté. */
export const ROLE = {
  args: { admin: true },
  argTypes: {
    admin: { name: "Administrateur", control: "boolean", description: "Décoché : rôle lecteur, sans les actions d'écriture." },
  },
};

/** Réglage des stories que le profil de l'installation change : la variable `PROFIL` (`src/config.rs`). */
export const PROFIL = {
  args: { profil: "entreprise" },
  argTypes: {
    profil: {
      name: "Profil",
      control: "inline-radio",
      options: ["entreprise", "particulier"],
      description: "« particulier » : sans les affectations ni le service des conducteurs.",
    },
  },
};
