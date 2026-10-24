// `templates/vehicules/liste.html`. Les listes de conducteurs, d'interventions et d'échéances
// sont bâties de la même façon.
import { CARBURANTS, CRIT_AIR, ROLE, STATUTS_VEHICULE, SUR_TELEPHONE, VEHICULES, boutonEnLigne, critAir, libelle, nombre, page, pastilleStatut, plaque, tableau } from "../fragments.js";

// Les réformés ne sont pas dans la liste de travail : repliés dessous, avec leur remise en service
// et leur suppression.
function liste({ admin, vehicules }) {
  const actifs = vehicules.filter((v) => v.statut !== "reforme");
  const reformes = vehicules.filter((v) => v.statut === "reforme");
  const entetes = ["Immatriculation", "Marque", "Modèle", "Mise en service", "Carburant", "Crit'Air", "Kilométrage", "Statut"];
  const ligne = (v) => [
    `<a href="/vehicules/${v.id}">${plaque(v.immatriculation, v.couleur)}</a>`,
    v.marque,
    v.modele,
    v.miseEnService,
    libelle(CARBURANTS, v.carburant),
    v.critAir === "non_classe" ? libelle(CRIT_AIR, v.critAir) : v.critAir ? critAir(v.critAir, libelle(CRIT_AIR, v.critAir)) : "",
    nombre(`${v.kilometrage} km`),
    pastilleStatut(STATUTS_VEHICULE, v.statut),
  ];
  const travail =
    vehicules.length === 0
      ? `<p class="vide">Aucun véhicule enregistré.</p>`
      : actifs.length === 0
        ? `<p class="vide">Aucun véhicule en service ni en maintenance.</p>`
        : tableau(entetes, actifs.map(ligne));
  const repliees = reformes.length
    ? `
<details class="repliee" id="reformes">
  <summary>Réformés (${reformes.length})</summary>
  ${tableau(
    [...entetes, ""],
    reformes.map((v) => [
      ...ligne(v),
      admin
        ? boutonEnLigne({ action: `/vehicules/${v.id}/remettre-en-service?retour=liste`, texte: "Remettre en service" }) +
          boutonEnLigne({
            action: `/vehicules/${v.id}/supprimer`,
            texte: "Supprimer",
            danger: true,
            confirmer: `Supprimer définitivement le véhicule ${v.immatriculation}, ses échéances, son entretien, ses affectations et ses documents ?`,
          })
        : "",
    ]),
  )}
</details>`
    : "";
  const corps = travail + repliees;
  return page({
    page: "vehicules",
    admin,
    contenu: `<div class="barre-titre">
  <h1>Véhicules</h1>
  ${admin ? `<a class="bouton" href="/vehicules/nouveau">Nouveau véhicule</a>` : ""}
</div>

${corps}`,
  });
}

export default {
  title: "Écrans/Véhicules/Liste",
  parameters: { layout: "fullscreen" },
  ...ROLE,
};

export const AvecVehicules = { name: "Avec véhicules", render: ({ admin }) => liste({ admin, vehicules: VEHICULES }) };

export const Vide = { render: ({ admin }) => liste({ admin, vehicules: [] }) };

export const SurTelephone = {
  name: "Avec véhicules, sur téléphone",
  globals: SUR_TELEPHONE,
  render: ({ admin }) => liste({ admin, vehicules: VEHICULES }),
};
