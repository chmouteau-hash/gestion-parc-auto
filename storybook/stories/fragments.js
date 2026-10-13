// Morceaux de page partagés par les stories.
//
// Storybook ne sait pas exécuter les gabarits Askama : chaque fonction ci-dessous recopie le HTML
// d'un gabarit de `templates/`, nommé dans son commentaire. Quand un gabarit change, sa copie est
// à retoucher ici. Les données sont fictives.

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

const MENU = [
  ["alertes", "/alertes", "Alertes"],
  ["vehicules", "/vehicules", "Véhicules"],
  ["conducteurs", "/conducteurs", "Conducteurs"],
  ["affectations", "/affectations", "Affectations"],
  ["interventions", "/interventions", "Entretien"],
  ["echeances", "/echeances", "Échéances"],
];

/** `Contexte::montre_affectations` et `montre_service` (`src/web/rendu.rs`) : seule une entreprise les voit. */
export function estEntreprise(profil) {
  return profil !== "particulier";
}

/** `templates/base.html`, de `<header>` à `</header>`. */
export function enTete({ page = "alertes", admin = true, login = "gestionnaire", profil = "entreprise" } = {}) {
  const lien = ([code, adresse, intitule]) =>
    `<a href="${adresse}"${page === code ? ' class="actif" aria-current="page"' : ""}>${intitule}</a>`;
  const liens = MENU.filter(([code]) => code !== "affectations" || estEntreprise(profil)).map(lien);
  if (admin) {
    liens.push(lien(["documents", "/documents/nouveau", "Déposer un document"]));
  }
  return `<header class="entete">
  <a class="marque" href="/alertes">Parc automobile</a>
  <nav class="menu">
    ${liens.join("\n    ")}
  </nav>
  <form class="session" method="post" action="/deconnexion">
    <span>${login} · ${admin ? "administrateur" : "lecture seule"}</span>
    <button type="submit" class="bouton-discret">Se déconnecter</button>
  </form>
</header>`;
}

/** `templates/base.html` en entier : le lien d'évitement, l'en-tête, puis le contenu de la page. */
export function page({ contenu, ...contexte }) {
  return `<a class="bouton lien-evitement" href="#contenu">Aller au contenu</a>
${enTete(contexte)}
<main class="contenu" id="contenu">
${contenu}
</main>`;
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
      `<a href="/documents/${d.id}/voir">${d.nom}</a>`,
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
