// `templates/recherche.html` : les résultats d'une recherche (véhicules, conducteurs), ou
// l'invitation à chercher.
import { PROFIL, STATUTS_VEHICULE, SUR_TELEPHONE, VEHICULES, page, pastilleStatut, plaque, recherche } from "../fragments.js";

const CONDUCTEURS = [
  { id: 1, prenom: "Léa", nom: "Martin", service: "Commercial", statut: "actif" },
  { id: 2, prenom: "Karim", nom: "Benali", service: "Technique", statut: "inactif" },
];

function initiales(c) {
  return `<span class="initiales" aria-hidden="true">${c.prenom[0]}${c.nom[0]}</span>`;
}

function resultats({ profil, question, vehicules, conducteurs }) {
  const entreprise = profil !== "particulier";
  let corps;
  if (!question) {
    corps = `<p class="aide">Tapez une immatriculation, une marque, un modèle, un numéro d'identification (VIN) ou le nom d'un conducteur.</p>`;
  } else if (vehicules.length === 0 && conducteurs.length === 0) {
    corps = `<p class="vide">Rien ne correspond à « ${question} ».</p>`;
  } else {
    const sectionVehicules =
      vehicules.length === 0
        ? ""
        : `<section>
  <h2 data-ico="voiture">Véhicules</h2>
  <ul class="liste-simple">
  ${vehicules.map((v) => `<li><a href="/vehicules/${v.id}">${plaque(v.immatriculation)}</a> ${v.marque} ${v.modele} ${pastilleStatut(STATUTS_VEHICULE, v.statut)}</li>`).join("\n  ")}
  </ul>
</section>`;
    const sectionConducteurs =
      conducteurs.length === 0
        ? ""
        : `<section>
  <h2 data-ico="personnes">Conducteurs</h2>
  <ul class="liste-simple">
  ${conducteurs
    .map(
      (c) =>
        `<li>${initiales(c)}<a href="/conducteurs/${c.id}">${c.prenom} ${c.nom}</a>${entreprise && c.service ? ` <span class="aide">${c.service}</span>` : ""} <span class="pastille statut-${c.statut}">${c.statut === "actif" ? "Actif" : "Inactif"}</span></li>`,
    )
    .join("\n  ")}
  </ul>
</section>`;
    corps = [sectionVehicules, sectionConducteurs].filter(Boolean).join("\n");
  }
  return page({
    page: "recherche",
    admin: true,
    profil,
    contenu: `<h1>Recherche</h1>
${recherche({ surLaPage: true, question })}

${corps}`,
  });
}

export default {
  title: "Écrans/Recherche",
  parameters: { layout: "fullscreen" },
  ...PROFIL,
};

export const AvecResultats = {
  name: "Avec résultats",
  render: ({ profil }) => resultats({ profil, question: "ma", vehicules: VEHICULES.slice(0, 2), conducteurs: CONDUCTEURS.slice(0, 1) }),
};

export const SansResultat = {
  name: "Sans résultat",
  render: ({ profil }) => resultats({ profil, question: "zzz", vehicules: [], conducteurs: [] }),
};

export const Vide = {
  name: "Avant de chercher",
  render: ({ profil }) => resultats({ profil, question: "", vehicules: [], conducteurs: [] }),
};

export const SurTelephone = {
  name: "Avec résultats, sur téléphone",
  globals: SUR_TELEPHONE,
  render: ({ profil }) => resultats({ profil, question: "ma", vehicules: VEHICULES.slice(0, 2), conducteurs: CONDUCTEURS }),
};
