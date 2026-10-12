// `templates/affectations/liste.html` : les affectations en cours d'abord, sur fond vert.
import { ETATS_AFFECTATION, ROLE, SUR_TELEPHONE, boutonEnLigne, page, pastille, tableau } from "../fragments.js";

const [EN_COURS, A_VENIR, TERMINEE] = ETATS_AFFECTATION;

const AFFECTATIONS = [
  { id: 1, vehiculeId: 1, immatriculation: "AB-123-CD", conducteurId: 1, conducteur: "Léa Martin", debut: "10/01/2026", fin: "—", etat: EN_COURS },
  { id: 2, vehiculeId: 2, immatriculation: "EF-456-GH", conducteurId: 2, conducteur: "Karim Benali", debut: "01/09/2026", fin: "01/12/2026", etat: EN_COURS },
  { id: 3, vehiculeId: 3, immatriculation: "JK-789-LM", conducteurId: 1, conducteur: "Léa Martin", debut: "01/11/2026", fin: "—", etat: A_VENIR },
  { id: 4, vehiculeId: 1, immatriculation: "AB-123-CD", conducteurId: 2, conducteur: "Karim Benali", debut: "15/03/2022", fin: "10/01/2026", etat: TERMINEE },
];

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

function liste({ admin, affectations }) {
  const corps =
    affectations.length === 0
      ? `<p class="vide">Aucune affectation.</p>`
      : tableau(
          ["Véhicule", "Conducteur", "Début", "Fin", "État", ""],
          affectations.map((a) => ({
            classe: a.etat === EN_COURS ? "ligne-active" : "",
            cellules: [
              `<a href="/vehicules/${a.vehiculeId}">${a.immatriculation}</a>`,
              `<a href="/conducteurs/${a.conducteurId}">${a.conducteur}</a>`,
              a.debut,
              a.fin,
              pastille(a.etat[1], a.etat[0]),
              admin ? actions(a) : "",
            ],
          })),
        );
  return page({
    page: "affectations",
    admin,
    contenu: `<div class="barre-titre">
  <h1>Affectations</h1>
  ${admin ? `<a class="bouton" href="/affectations/nouvelle">Nouvelle affectation</a>` : ""}
</div>
<p class="sous-titre">Les affectations en cours apparaissent en premier : celles qui ont commencé et dont la date de fin n'est pas encore atteinte. Viennent ensuite celles à venir, puis les terminées. Un véhicule peut avoir plusieurs conducteurs, et un conducteur plusieurs véhicules.</p>

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
