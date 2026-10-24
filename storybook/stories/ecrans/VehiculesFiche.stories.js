// `templates/vehicules/fiche.html` : l'écran le plus fourni. La fiche d'un conducteur reprend la
// même construction, avec moins de sections.
import {
  CARBURANTS,
  CRIT_AIR,
  DOCUMENTS,
  ETATS_AFFECTATION,
  ETATS_ECHEANCE,
  PHOTO_FICTIVE,
  PROFIL,
  ROLE,
  STATUTS_VEHICULE,
  SUR_TELEPHONE,
  VEHICULES,
  boutonEnLigne,
  boutonImprimer,
  couleur,
  critAir,
  depot,
  dossiersDocuments,
  estEntreprise,
  libelle,
  nombre,
  page,
  pastille,
  pastilleStatut,
  plaque,
  releve,
  tableau,
  trajet,
} from "../fragments.js";

const [EN_COURS, , TERMINEE] = ETATS_AFFECTATION;
const [DEPASSEE, A_VENIR, A_TRAITER, TRAITEE] = ETATS_ECHEANCE;

const HISTORIQUE = {
  affectationsActives: [{ conducteurId: 1, nom: "Léa Martin", service: "Logistique", periode: "depuis le 10/01/2026" }],
  echeances: [
    { id: 1, type: "Contrôle technique", date: "02/10/2026", etat: DEPASSEE },
    { id: 4, type: "Assurance", date: "21/10/2026", etat: A_VENIR },
    { id: 5, type: "À faire : Câble d'embrayage", constat: "câble qui accroche", date: "30/10/2026", etat: A_VENIR },
    { id: 2, type: "Assurance", date: "31/12/2026", etat: A_TRAITER },
    { id: 3, type: "Entretien périodique", date: "15/03/2026", etat: TRAITEE },
  ],
  // Le plan d'entretien (#94, bloc 2) : la dernière fois et la prochaine se calculent à partir de
  // l'entretien ; la barre remplit la part de l'intervalle parcourue, la borne qui presse en gras.
  plan: [
    { id: 1, code: "vidange", operation: "Vidange", periodicite: "tous les 15 000 km ou tous les 12 mois", derniere: "12/09/2026 à 47 850 km", km: "62 850 km · dans 13 400 km", date: "12/09/2027 · dans 334 jours", kmPresse: false, etat: ["plan-a_jour", "À jour"], avancement: 10 },
    { id: 4, code: "filtre_air", operation: "Filtre à air", periodicite: "tous les 30 000 km", derniere: "03/04/2025 à 18 300 km", km: "48 300 km · dans 450 km", kmPresse: true, etat: ["plan-a_venir", "À venir"], avancement: 100 },
    { id: 2, code: "pression_pneus", controle: true, operation: "Pression des pneus", periodicite: "tous les mois", derniere: "12/09/2026 à 47 850 km", date: "12/10/2026 · depuis 11 jours", kmPresse: false, etat: ["plan-depassee", "Dépassée"], avancement: 100 },
    { id: 3, code: "courroie_distribution", operation: "Courroie de distribution", periodicite: "tous les 120 000 km ou tous les 60 mois", derniere: "Inconnue", etat: ["plan-inconnue", "Dernière fois inconnue"] },
  ],
  interventions: [
    { id: 1, date: "12/09/2026", type: "Révision", operations: "Vidange (réf. 5W30, 45,00 €) · Filtre à huile (réf. W712, 12,50 €) · Filtre à air", kilometrage: 47850, cout: "189,90 €", prestataire: "Garage du Centre", notes: "Contrôle des freins.", facture: 4 },
    { id: 2, date: "03/04/2026", type: "Pneumatiques", operations: "Pneus (× 4)", kilometrage: 41200, cout: "412,00 €", prestataire: "Pneus Express", notes: "" },
  ],
  depenses: [
    { id: 2, date: "13/10/2026", type: "Péage", detail: "Lyon – Grenoble", montant: "12,30 €", notes: "" },
    { id: 1, date: "12/10/2026", type: "Carburant", detail: "45,2 L Gazole (B7) · 61 200 km · 6,2 L/100 km · Station du Centre", montant: "68,40 €", notes: "" },
  ],
  // Le coût (#126, bloc 3, et #94, bloc 6) : un seul bloc, calculé sur toutes les dépenses et toutes
  // les interventions du véhicule : entretien (dont les pièces), carburant et péages.
  resumeCout: {
    annee: 2026,
    cetteAnnee: "Entretien 601,90 € (pièces 57,50 €&nbsp;· reste 544,40 €)&nbsp;· Carburant 412,80 €&nbsp;· Péages 30,90 €&nbsp;· <strong>Total 1 045,60 €</strong>",
    douzeMois: "Entretien 840,00 € (pièces 210,50 €&nbsp;· reste 629,50 €)&nbsp;· Carburant 540,20 €&nbsp;· Péages 48,30 €&nbsp;· <strong>Total 1 428,50 €</strong>",
    consommation: "6,3 L/100 km",
    consommationDetail: "sur 2 750 km, 4 pleins complets mesurés",
    cout: "0,52 €/km",
    coutDetail: "sur 2 750 km, douze derniers mois, entretien compris",
  },
  affectations: [
    { conducteurId: 1, nom: "Léa Martin", service: "Logistique", debut: "10/01/2026", fin: "—", etat: EN_COURS },
    { conducteurId: 2, nom: "Karim Benali", service: "Atelier", debut: "15/03/2022", fin: "10/01/2026", etat: TERMINEE },
  ],
  documents: DOCUMENTS,
};

// Après un contrôle technique défavorable (#107) : la contre-visite remplace le contrôle, et sa
// ligne propose de noter un défaut à corriger ; un défaut est déjà noté.
const DEFAUT = "Défaut relevé au contrôle technique du 21/10/2026";
const EN_CONTRE_VISITE = [
  { id: 1, type: "Contrôle technique", date: "12/11/2026", etat: ["echeance-remplacee", "Remplacée par la contre-visite"] },
  { id: 6, type: "Contre-visite", date: "21/12/2026", etat: A_TRAITER, defaut: { dateLimite: "2026-12-21", constat: DEFAUT } },
  { id: 7, type: "À faire : Plaquettes de frein", constat: DEFAUT, date: "21/12/2026", etat: A_TRAITER },
  { id: 4, type: "Assurance", date: "21/10/2026", etat: A_VENIR },
];

const SANS_HISTORIQUE = { affectationsActives: [], echeances: [], plan: [], interventions: [], depenses: [], resumeCout: null, affectations: [], documents: [] };

function conducteursActuels({ admin, vehicule, affectationsActives }) {
  const affectable = admin && vehicule.statut !== "reforme";
  const nouvelle = `/affectations/nouvelle?vehicule_id=${vehicule.id}`;
  if (affectationsActives.length === 0) {
    return `<em>aucun</em>${affectable ? `<span class="action-fiche"> · <a href="${nouvelle}">Affecter</a></span>` : ""}`;
  }
  const lignes = affectationsActives.map((a) => `<div><a href="/conducteurs/${a.conducteurId}">${a.nom}</a>, ${a.service} ${a.periode}</div>`);
  if (affectable) {
    lignes.push(`<div class="action-fiche"><a href="${nouvelle}">Affecter un autre conducteur</a></div>`);
  }
  return lignes.join("\n    ");
}

// Le bloc « Photos » (#66) : toutes les photos du véhicule en vignettes, la photo de la fiche en
// premier. À partir de deux photos seulement ; l'administrateur choisit celle de la fiche.
function blocPhotos({ photos, admin, vehicule }) {
  if (photos.length < 2) {
    return "";
  }
  const vignettes = photos.map((date, i) => {
    const id = 10 + i;
    const legende =
      i === 0
        ? `<strong class="photo-choisie">Photo de la fiche</strong>`
        : admin
          ? `<form method="post" action="/documents/${id}/mettre-en-tete" class="en-ligne"><button type="submit" class="bouton-discret" aria-label="Mettre en tête la photo du ${date}">Mettre en tête</button></form>`
          : "";
    return `<li class="photo-vignette${i === 0 ? " en-tete" : ""}">
      <a href="/documents/${id}"><img src="${PHOTO_FICTIVE}" alt="Photo déposée le ${date}" loading="lazy"></a>
      <div class="photo-legende"><span>${date}</span>${legende}</div>
    </li>`;
  });
  return `<section id="photos">
  <div class="barre-titre">
    <h2>Photos (${photos.length})</h2>
    ${admin ? `<a class="bouton-discret" href="/documents/nouveau?vehicule_id=${vehicule.id}&type_document=photo_vehicule">Ajouter une photo</a>` : ""}
  </div>
  <ul class="photos">
    ${vignettes.join("\n    ")}
  </ul>
</section>`;
}

function fiche({ admin, profil, vehicule, affectationsActives, echeances, plan = [], interventions, depenses = [], resumeCout = null, affectations, documents, erreurDepot = "", photo = false, photos = [] }) {
  const v = vehicule;
  const avecAffectations = estEntreprise(profil);
  // La photo du véhicule en tête de fiche ; sans photo, la silhouette et le lien pour en prendre une.
  const fichePhoto = photo
    ? `<a href="/documents/4"><img src="${PHOTO_FICTIVE}" alt="Photo de ${v.immatriculation} (${v.marque} ${v.modele})"></a>`
    : `<span class="silhouette${v.couleur ? ` couleur-${v.couleur}` : ""}" aria-hidden="true"></span>${admin ? `\n  <a class="aide" href="/documents/nouveau?vehicule_id=${v.id}&type_document=photo_vehicule">Ajouter une photo</a>` : ""}`;
  // « Imprimer » est là pour tous ; « Modifier » et « Supprimer » pour l'administrateur, et
  // « Remettre en service » sur un réformé. Un réformé n'a plus à se faire conseiller le statut
  // « Réformé » dans la boîte de suppression.
  const reforme = v.statut === "reforme";
  const supprimer = boutonEnLigne({
    action: `/vehicules/${v.id}/supprimer`,
    texte: "Supprimer",
    danger: true,
    confirmer: reforme
      ? `Supprimer le véhicule ${v.immatriculation} et tout son historique ?`
      : `Supprimer le véhicule ${v.immatriculation} et tout son historique ? S'il a réellement existé, passez-le plutôt au statut « Réformé » (Modifier, champ Statut) : il reste dans l'historique.`,
    alternative: reforme ? null : { href: `/vehicules/${v.id}/modifier`, libelle: "Modifier le statut" },
  });
  const remettre = boutonEnLigne({ action: `/vehicules/${v.id}/remettre-en-service?retour=fiche`, texte: "Remettre en service" });
  const actions = `
  <div class="actions">
    ${admin ? `<a class="bouton" href="/vehicules/${v.id}/modifier">Modifier</a>\n    ` : ""}${admin && reforme ? `${remettre}\n    ` : ""}${boutonImprimer()}${admin ? `\n    ${supprimer}` : ""}
  </div>`;

  const tableauEcheances =
    echeances.length === 0
      ? `<p class="vide">Aucune échéance.</p>`
      : tableau(
          ["Type", "Date limite", "Statut", ""],
          echeances.map((e) => [
            e.constat ? `${e.type}<br><small>${e.constat}</small>` : e.type,
            e.date,
            pastille(e.etat[1], e.etat[0]),
            admin
              ? `<a href="/echeances/${e.id}/modifier">Modifier</a>${e.defaut ? `\n<a class="bouton-discret" href="/echeances/nouvelle?vehicule_id=${v.id}&amp;type_echeance=a_faire&amp;date_limite=${e.defaut.dateLimite}&amp;constat=${encodeURIComponent(e.defaut.constat)}&amp;retour=vehicule">Noter un défaut à corriger</a>` : ""}${e.etat !== TRAITEE ? `\n${boutonEnLigne({ action: `/echeances/${e.id}/traiter?retour=vehicule`, texte: "Marquer traitée" })}` : ""}`
              : "",
          ]),
        );

  const inconnue = (p) => p.etat[0] === "plan-inconnue";
  const tableauPlan =
    plan.length === 0
      ? `<p class="vide">Aucune opération suivie. Le plan dit quand refaire la vidange, les filtres ou la pression des pneus.</p>`
      : tableau(
          ["Opération", "Périodicité", "Dernière fois", "Prochaine", "État", ""],
          plan.map((p) => [
            p.operation,
            p.periodicite,
            p.derniere,
            inconnue(p)
              ? `<small>Notez-la en ajoutant l'intervention où l'opération a été faite.</small>`
              : `${p.km ? `<div${p.kmPresse ? ` class="presse"` : ""}>${p.km}</div>` : ""}${p.date ? `<div${p.kmPresse ? "" : ` class="presse"`}>${p.date}</div>` : ""}`,
            `${pastille(p.etat[1], p.etat[0])}${inconnue(p) ? "" : ` ${trajet(p.etat[0], p.avancement)}`}`,
            admin
              ? `<a href="/plan-entretien/${p.id}/modifier">Modifier</a>
<a class="bouton-discret" href="/interventions/nouvelle?vehicule_id=${v.id}&amp;kilometrage=${v.kilometrage}&amp;type_intervention=${p.controle ? "entretien_courant" : "revision"}&amp;operation=${p.code}&amp;retour=vehicule">C'est fait</a>
${boutonEnLigne({ action: `/plan-entretien/${p.id}/supprimer`, texte: "Ne plus suivre", danger: true, confirmer: `Ne plus suivre « ${p.operation} » ? Les interventions restent dans l'entretien.` })}`
              : "",
          ]),
        );

  const tableauInterventions =
    interventions.length === 0
      ? `<p class="vide">Aucune intervention.</p>`
      : tableau(
          ["Date", "Type", "Opérations", "Kilométrage", "Coût", "Prestataire", "Notes", ""],
          interventions.map((i) => [
            i.date,
            i.type,
            `<span class="operations-faites">${i.operations}</span>`,
            nombre(`${i.kilometrage} km`),
            nombre(i.cout),
            i.prestataire,
            i.notes,
            `${i.facture ? `<a href="/documents/${i.facture}">Facture</a> ` : ""}${admin ? `<a href="/interventions/${i.id}/modifier">Modifier</a>` : ""}`,
          ]),
        );

  // Les dépenses (#126) : les dix dernières, le détail en une ligne, le compteur jamais pré-rempli.
  const tableauDepenses =
    depenses.length === 0
      ? `<p class="vide">Aucune dépense.</p>`
      : tableau(
          ["Date", "Type", "Détail", "Montant", "Notes", ""],
          depenses.map((d) => [d.date, d.type, d.detail, nombre(d.montant), d.notes, admin ? `<a href="/depenses/${d.id}/modifier">Modifier</a>` : ""]),
        );

  // Le coût (#126, bloc 3, et #94, bloc 6) : un seul bloc après les dépenses ; il y en a un dès
  // qu'il y a de quoi le calculer, même sans dépense (des interventions suffisent).
  const blocCout = resumeCout
    ? `<section>
  <h2>Coût</h2>
  <dl class="fiche">
    <dt>Cette année (${resumeCout.annee})</dt>
    <dd>${resumeCout.cetteAnnee}</dd>
    <dt>Douze derniers mois</dt>
    <dd>${resumeCout.douzeMois}</dd>
    <dt>Consommation moyenne</dt>
    <dd><strong>${resumeCout.consommation}</strong> <small>${resumeCout.consommationDetail}</small></dd>
    <dt>Coût au kilomètre</dt>
    <dd><strong>${resumeCout.cout}</strong> <small>${resumeCout.coutDetail}</small></dd>
  </dl>
  <p class="aide">« Pièces » : les prix saisis sur les lignes « ce qui a été fait » des interventions. « Reste » : le reste de la facture (main-d'œuvre, ce qui n'est pas détaillé).</p>
</section>
`
    : "";

  const tableauAffectations =
    affectations.length === 0
      ? `<p class="vide">Jamais affecté.</p>`
      : tableau(
          ["Conducteur", "Début", "Fin", "État"],
          affectations.map((a) => [`<a href="/conducteurs/${a.conducteurId}">${a.nom}</a>, ${a.service}`, a.debut, a.fin, pastille(a.etat[1], a.etat[0])]),
        );

  const conducteurs = avecAffectations
    ? `
  <dt>${affectationsActives.length > 1 ? "Conducteurs actuels" : "Conducteur actuel"}</dt>
  <dd>
    ${conducteursActuels({ admin, vehicule, affectationsActives })}
  </dd>`
    : "";

  const historique = avecAffectations
    ? `
<section>
  <h2>Historique des affectations</h2>
  ${tableauAffectations}
</section>
`
    : "";

  return page({
    page: "vehicules",
    admin,
    profil,
    contenu: `<a class="retour" href="/vehicules">Véhicules</a>
<div class="barre-titre">
  <h1>${plaque(v.immatriculation, v.couleur)} <small>${v.marque} ${v.modele}</small></h1>${actions}
</div>
${erreurDepot ? `\n<p class="message-erreur" role="alert">Document non déposé. ${erreurDepot}</p>\n` : ""}
<div class="fiche-entete">
<figure class="fiche-photo">
  ${fichePhoto}
</figure>
<dl class="fiche">
  <dt>Statut</dt><dd>${pastilleStatut(STATUTS_VEHICULE, v.statut)}</dd>
  <dt>Numéro d'identification (VIN)</dt><dd>${v.vin ? `<span class="vin">${v.vin}</span>` : "<em>non renseigné</em>"}</dd>
  <dt>Couleur</dt><dd>${v.couleur ? couleur(v.couleur) : "<em>non renseignée</em>"}</dd>
  <dt>Mise en service</dt><dd>${v.miseEnService}</dd>
  <dt>Carburant</dt><dd>${libelle(CARBURANTS, v.carburant)}</dd>
  <dt>Vignette Crit'Air</dt><dd>${v.critAir ? `${critAir(v.critAir)} ${libelle(CRIT_AIR, v.critAir)}` : `<em>non renseignée</em>${v.critAirPropose ? ` · d'après le carburant et la mise en service : ${libelle(CRIT_AIR, v.critAirPropose)}, à vérifier sur le certificat` : ""}`}</dd>
  <dt>Kilométrage courant</dt><dd>${v.kilometrage} km <a href="/vehicules/${v.id}/compteur">historique</a>${admin ? ` ${releve(v.id, v.kilometrage)}` : ""}</dd>${conducteurs}
</dl>
</div>

<section>
  <div class="barre-titre">
    <h2>Échéances</h2>
    ${admin ? `<div class="actions">
      <a class="bouton-discret" href="/echeances/nouvelle?vehicule_id=${v.id}&amp;type_echeance=a_faire">Noter ce qu'il y a à faire</a>
      <a class="bouton-discret" href="/echeances/nouvelle?vehicule_id=${v.id}">Ajouter une échéance</a>
    </div>` : ""}
  </div>
  ${tableauEcheances}
</section>

<section id="plan">
  <div class="barre-titre">
    <h2>Plan d'entretien</h2>
    ${admin ? `<div class="actions">${plan.length === 0 ? `<a class="bouton-discret" href="/plan-entretien/depart?vehicule_id=${v.id}">Proposer un plan</a>\n      ` : ""}<a class="bouton-discret" href="/plan-entretien/nouveau?vehicule_id=${v.id}">Suivre une opération</a></div>` : ""}
  </div>
  ${tableauPlan}
</section>

<section>
  <div class="barre-titre">
    <h2>Entretien</h2>
    ${admin ? `<a class="bouton-discret" href="/interventions/nouvelle?vehicule_id=${v.id}&kilometrage=${v.kilometrage}">Ajouter une intervention</a>` : ""}
  </div>
  ${tableauInterventions}
</section>

<section>
  <div class="barre-titre">
    <h2>Dépenses</h2>
    ${admin ? `<div class="actions">
      <a class="bouton-discret" href="/depenses/nouvelle?vehicule_id=${v.id}&amp;type_depense=carburant">Noter un plein</a>
      <a class="bouton-discret" href="/depenses/nouvelle?vehicule_id=${v.id}&amp;type_depense=peage">Noter un péage</a>
    </div>` : ""}
  </div>
  ${tableauDepenses}
</section>
${blocCout}${historique}
${blocPhotos({ photos, admin, vehicule: v })}
<section id="documents">
  <h2>Documents</h2>
  ${dossiersDocuments({ documents, admin, vehiculeId: v.id })}
  ${admin ? depot({ action: `/vehicules/${v.id}/documents` }) : ""}
</section>`,
  });
}

export default {
  title: "Écrans/Véhicules/Fiche",
  parameters: { layout: "fullscreen" },
  args: { ...ROLE.args, ...PROFIL.args },
  argTypes: { ...ROLE.argTypes, ...PROFIL.argTypes },
};

export const Complete = {
  name: "Complète",
  render: ({ admin, profil }) => fiche({ admin, profil, vehicule: VEHICULES[0], ...HISTORIQUE, photo: true }),
};

// Plusieurs photos : le bloc « Photos » les montre toutes, la photo de la fiche cerclée en premier.
export const PlusieursPhotos = {
  name: "Plusieurs photos",
  render: ({ admin, profil }) =>
    fiche({
      admin,
      profil,
      vehicule: VEHICULES[0],
      ...HISTORIQUE,
      photo: true,
      photos: ["14/10/2026 18:02", "12/10/2026 09:41", "03/09/2026 16:20"],
    }),
};

export const ContreVisite = {
  name: "Contre-visite à faire",
  render: ({ admin, profil }) => fiche({ admin, profil, vehicule: VEHICULES[0], ...HISTORIQUE, echeances: EN_CONTRE_VISITE }),
};

// Véhicule tout juste créé : chaque section dit qu'elle est vide.
export const SansHistorique = {
  name: "Sans historique",
  render: ({ admin, profil }) => fiche({ admin, profil, vehicule: VEHICULES[2], ...SANS_HISTORIQUE }),
};

// Un véhicule réformé ne propose plus d'affecter un conducteur, mais de le remettre en service.
export const Reforme = {
  name: "Réformé",
  render: ({ admin, profil }) =>
    fiche({ admin, profil, vehicule: VEHICULES[3], ...SANS_HISTORIQUE, affectations: [{ ...HISTORIQUE.affectations[1], fin: "01/07/2025" }] }),
};

export const DepotRefuse = {
  name: "Dépôt refusé",
  render: ({ admin, profil }) =>
    fiche({ admin, profil, vehicule: VEHICULES[0], ...HISTORIQUE, erreurDepot: "Le fichier dépasse la taille maximale de 10 Mo." }),
};

// Profil « particulier » : ni conducteur actuel, ni historique des affectations.
export const Particulier = {
  name: "Complète, profil particulier",
  args: { profil: "particulier" },
  render: ({ admin, profil }) => fiche({ admin, profil, vehicule: VEHICULES[0], ...HISTORIQUE }),
};

export const SurTelephone = {
  name: "Complète, sur téléphone",
  globals: SUR_TELEPHONE,
  render: ({ admin, profil }) => fiche({ admin, profil, vehicule: VEHICULES[0], ...HISTORIQUE }),
};
