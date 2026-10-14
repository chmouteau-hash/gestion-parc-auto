// `templates/vehicules/fiche.html` : l'écran le plus fourni. La fiche d'un conducteur reprend la
// même construction, avec moins de sections.
import {
  CARBURANTS,
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
  depot,
  estEntreprise,
  libelle,
  listeDocuments,
  nombre,
  page,
  pastille,
  pastilleStatut,
  releve,
  tableau,
} from "../fragments.js";

const [EN_COURS, , TERMINEE] = ETATS_AFFECTATION;
const [DEPASSEE, A_VENIR, A_TRAITER, TRAITEE] = ETATS_ECHEANCE;

const HISTORIQUE = {
  affectationsActives: [{ conducteurId: 1, nom: "Léa Martin", service: "Logistique", periode: "depuis le 10/01/2026" }],
  echeances: [
    { id: 1, type: "Contrôle technique", date: "02/10/2026", etat: DEPASSEE },
    { id: 4, type: "Assurance", date: "21/10/2026", etat: A_VENIR },
    { id: 2, type: "Assurance", date: "31/12/2026", etat: A_TRAITER },
    { id: 3, type: "Entretien périodique", date: "15/03/2026", etat: TRAITEE },
  ],
  interventions: [
    { id: 1, date: "12/09/2026", type: "Révision", kilometrage: 47850, cout: "189,90 €", prestataire: "Garage du Centre", notes: "Vidange, filtres, contrôle des freins." },
    { id: 2, date: "03/04/2026", type: "Pneumatiques", kilometrage: 41200, cout: "412,00 €", prestataire: "Pneus Express", notes: "" },
  ],
  affectations: [
    { conducteurId: 1, nom: "Léa Martin", service: "Logistique", debut: "10/01/2026", fin: "—", etat: EN_COURS },
    { conducteurId: 2, nom: "Karim Benali", service: "Atelier", debut: "15/03/2022", fin: "10/01/2026", etat: TERMINEE },
  ],
  documents: DOCUMENTS,
};

const SANS_HISTORIQUE = { affectationsActives: [], echeances: [], interventions: [], affectations: [], documents: [] };

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

function fiche({ admin, profil, vehicule, affectationsActives, echeances, interventions, affectations, documents, erreurDepot = "", photo = false }) {
  const v = vehicule;
  const avecAffectations = estEntreprise(profil);
  // La photo du véhicule en tête de fiche ; sans photo, la silhouette et le lien pour en prendre une.
  const fichePhoto = photo
    ? `<a href="/documents/4"><img src="${PHOTO_FICTIVE}" alt="Photo de ${v.immatriculation} (${v.marque} ${v.modele})"></a>`
    : `<span class="silhouette" aria-hidden="true"></span>${admin ? `\n  <a class="aide" href="/documents/nouveau?vehicule_id=${v.id}&type_document=photo_vehicule">Ajouter une photo</a>` : ""}`;
  // « Imprimer » est là pour tous ; « Modifier » et « Supprimer » pour l'administrateur.
  const supprimer = boutonEnLigne({
    action: `/vehicules/${v.id}/supprimer`,
    texte: "Supprimer",
    danger: true,
    confirmer: `Supprimer le véhicule ${v.immatriculation} et tout son historique ? Préférez le statut « Réformé » s'il a réellement existé.`,
  });
  const actions = `
  <div class="actions">
    ${admin ? `<a class="bouton" href="/vehicules/${v.id}/modifier">Modifier</a>\n    ` : ""}${boutonImprimer()}${admin ? `\n    ${supprimer}` : ""}
  </div>`;

  const tableauEcheances =
    echeances.length === 0
      ? `<p class="vide">Aucune échéance.</p>`
      : tableau(
          ["Type", "Date limite", "Statut", ""],
          echeances.map((e) => [
            e.type,
            e.date,
            pastille(e.etat[1], e.etat[0]),
            admin
              ? `<a href="/echeances/${e.id}/modifier">Modifier</a>${e.etat !== TRAITEE ? `\n${boutonEnLigne({ action: `/echeances/${e.id}/traiter?retour=vehicule`, texte: "Marquer traitée" })}` : ""}`
              : "",
          ]),
        );

  const tableauInterventions =
    interventions.length === 0
      ? `<p class="vide">Aucune intervention.</p>`
      : tableau(
          ["Date", "Type", "Kilométrage", "Coût", "Prestataire", "Notes", ""],
          interventions.map((i) => [
            i.date,
            i.type,
            nombre(`${i.kilometrage} km`),
            nombre(i.cout),
            i.prestataire,
            i.notes,
            admin ? `<a href="/interventions/${i.id}/modifier">Modifier</a>` : "",
          ]),
        );

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
  <h1>${v.immatriculation} <small>${v.marque} ${v.modele}</small></h1>${actions}
</div>
${erreurDepot ? `\n<p class="message-erreur" role="alert">Document non déposé. ${erreurDepot}</p>\n` : ""}
<div class="fiche-entete">
<figure class="fiche-photo">
  ${fichePhoto}
</figure>
<dl class="fiche">
  <dt>Statut</dt><dd>${pastilleStatut(STATUTS_VEHICULE, v.statut)}</dd>
  <dt>Numéro d'identification (VIN)</dt><dd>${v.vin ? `<span class="vin">${v.vin}</span>` : "<em>non renseigné</em>"}</dd>
  <dt>Mise en service</dt><dd>${v.miseEnService}</dd>
  <dt>Carburant</dt><dd>${libelle(CARBURANTS, v.carburant)}</dd>
  <dt>Kilométrage courant</dt><dd>${v.kilometrage} km${admin ? ` ${releve(v.id, v.kilometrage)}` : ""}</dd>${conducteurs}
</dl>
</div>

<section>
  <div class="barre-titre">
    <h2>Échéances</h2>
    ${admin ? `<a class="bouton-discret" href="/echeances/nouvelle?vehicule_id=${v.id}">Ajouter une échéance</a>` : ""}
  </div>
  ${tableauEcheances}
</section>

<section>
  <div class="barre-titre">
    <h2>Entretien</h2>
    ${admin ? `<a class="bouton-discret" href="/interventions/nouvelle?vehicule_id=${v.id}&kilometrage=${v.kilometrage}">Ajouter une intervention</a>` : ""}
  </div>
  ${tableauInterventions}
</section>
${historique}
<section id="documents">
  <h2>Documents</h2>
  ${listeDocuments({ documents, admin })}
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

// Véhicule tout juste créé : chaque section dit qu'elle est vide.
export const SansHistorique = {
  name: "Sans historique",
  render: ({ admin, profil }) => fiche({ admin, profil, vehicule: VEHICULES[2], ...SANS_HISTORIQUE }),
};

// Un véhicule réformé ne propose plus d'affecter un conducteur.
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
