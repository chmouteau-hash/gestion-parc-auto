// `templates/affectations/liste.html` : les affectations en cours d'abord, sur fond vert, puis
// celles à venir ; les terminées sont repliées dessous. Réservé à Autoclair Pro.
import { ETATS_AFFECTATION, ROLE, SUR_TELEPHONE, VEHICULES, boutonEnLigne, ecranVide, page, pastille, plaque, tableau } from "../fragments.js";

const [EN_COURS, A_VENIR, TERMINEE] = ETATS_AFFECTATION;

const AFFECTATIONS = [
  { id: 1, vehiculeId: 1, immatriculation: "AB-123-CD", conducteurId: 1, conducteur: "Léa Martin", service: "Logistique", debut: "10/01/2026", fin: "—", etat: EN_COURS },
  { id: 2, vehiculeId: 2, immatriculation: "EF-456-GH", conducteurId: 2, conducteur: "Karim Benali", service: "Atelier", debut: "01/09/2026", fin: "01/12/2026", etat: EN_COURS },
  { id: 3, vehiculeId: 3, immatriculation: "JK-789-LM", conducteurId: 1, conducteur: "Léa Martin", service: "Logistique", debut: "01/11/2026", fin: "—", etat: A_VENIR },
  { id: 4, vehiculeId: 1, immatriculation: "AB-123-CD", conducteurId: 2, conducteur: "Karim Benali", service: "Atelier", debut: "15/03/2022", fin: "10/01/2026", etat: TERMINEE },
];

// La couleur de repère de la plaque est celle du véhicule.
const couleur = (a) => VEHICULES.find((v) => v.id === a.vehiculeId)?.couleur ?? "";

function actions(a) {
  return [
    `<a href="/affectations/${a.id}/modifier">Modifier</a>`,
    a.etat === EN_COURS ? boutonEnLigne({ action: `/affectations/${a.id}/cloturer`, texte: "Terminer aujourd'hui" }) : "",
    boutonEnLigne({
      action: `/affectations/${a.id}/supprimer`,
      texte: "Supprimer",
      danger: true,
      confirmer: "Supprimer cette affectation de l'historique ?",
    }),
  ].join("\n");
}

// La macro `tableau_affectations` de `templates/macros.html`.
const tableauAffectations = (admin, affectations) =>
  tableau(
    ["Véhicule", "Conducteur", "Début", "Fin", "État", ""],
    affectations.map((a) => ({
      classe: a.etat === EN_COURS ? "ligne-active" : "",
      cellules: [
        `<a href="/vehicules/${a.vehiculeId}">${plaque(a.immatriculation, couleur(a))}</a>`,
        `<a href="/conducteurs/${a.conducteurId}">${a.conducteur}</a>, ${a.service}`,
        a.debut,
        a.fin,
        pastille(a.etat[1], a.etat[0]),
        admin ? actions(a) : "",
      ],
    })),
  );

function liste({ admin, affectations }) {
  const enCours = affectations.filter((a) => a.etat !== TERMINEE);
  const terminees = affectations.filter((a) => a.etat === TERMINEE);
  const corps =
    affectations.length === 0
      ? ecranVide("Aucune affectation.", admin ? { lien: "/affectations/nouvelle", action: "Nouvelle affectation" } : {})
      : (enCours.length === 0 ? `<p class="vide">Aucune affectation en cours ni à venir.</p>` : tableauAffectations(admin, enCours)) +
        (terminees.length === 0
          ? ""
          : `
<details class="repliee">
  <summary>Terminées (${terminees.length})</summary>
  ${tableauAffectations(admin, terminees)}
</details>`);
  const actionsTitre =
    affectations.length === 0
      ? ""
      : `<div class="actions">
    ${admin ? `<a class="bouton" href="/affectations/nouvelle">Nouvelle affectation</a>` : ""}
    <a class="bouton-discret export-csv" href="/affectations.csv">Exporter (CSV)</a>
  </div>`;
  return page({
    page: "affectations",
    admin,
    contenu: `<div class="barre-titre">
  <h1>Affectations</h1>
  ${actionsTitre}
</div>
<p class="sous-titre">Les affectations en cours (commencées, date de fin pas encore atteinte) apparaissent en premier, puis celles à venir. Les terminées sont repliées sous la liste. Un véhicule peut avoir plusieurs conducteurs, et un conducteur plusieurs véhicules.</p>

${corps}`,
  });
}

export default {
  title: "Écrans/Affectations/Liste",
  parameters: { layout: "fullscreen" },
  ...ROLE,
};

export const AvecAffectations = { name: "Avec affectations", render: ({ admin }) => liste({ admin, affectations: AFFECTATIONS }) };

export const Vide = { render: ({ admin }) => liste({ admin, affectations: [] }) };

export const SurTelephone = {
  name: "Avec affectations, sur téléphone",
  globals: SUR_TELEPHONE,
  render: ({ admin }) => liste({ admin, affectations: AFFECTATIONS }),
};
