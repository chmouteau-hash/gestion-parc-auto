// `templates/vehicules/liste.html`. Les listes de conducteurs, d'interventions et d'échéances
// sont bâties de la même façon.
import { CARBURANTS, ROLE, STATUTS_VEHICULE, SUR_TELEPHONE, VEHICULES, libelle, nombre, page, pastilleStatut, tableau } from "../fragments.js";

function liste({ admin, vehicules }) {
  const corps =
    vehicules.length === 0
      ? `<p class="vide">Aucun véhicule enregistré.</p>`
      : tableau(
          ["Immatriculation", "Marque", "Modèle", "Mise en service", "Carburant", "Kilométrage", "Statut"],
          vehicules.map((v) => [
            `<a href="/vehicules/${v.id}">${v.immatriculation}</a>`,
            v.marque,
            v.modele,
            v.miseEnService,
            libelle(CARBURANTS, v.carburant),
            nombre(`${v.kilometrage} km`),
            pastilleStatut(STATUTS_VEHICULE, v.statut),
          ]),
        );
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
