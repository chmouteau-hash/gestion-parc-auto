// Les repères visuels de l'accueil (`templates/macros.html`) : la borne, la barre de trajet, la
// plaque et l'écran vide. Leurs couleurs sont celles des états de la charte.
import { borne, ecranVide, plaque, trajet } from "../fragments.js";

export default {
  title: "Primitives/Repères",
};

// `ResumeVehicule::classe_borne`, `borne_valeur`, `borne_unite` (`src/metier/accueil.rs`).
export const Bornes = {
  name: "Borne : jours restants",
  render: () => `<div style="display: flex; gap: 12px">
${borne("borne-depassee", "5", "retard")}
${borne("borne-a-venir", "15", "jours")}
${borne("borne-a-venir", "1", "jour")}
${borne("borne-a-jour", "120", "jours")}
${borne("borne-neutre", "—", "")}
</div>`,
};

// `Alerte::avancement_dizaine` : la barre se remplit sur l'horizon des alertes.
export const Trajets = {
  name: "Barre de trajet",
  render: () => `<div style="display: grid; gap: 12px; max-width: 320px">
${trajet("alerte-a-venir", 0)}
${trajet("alerte-a-venir", 50)}
${trajet("alerte-a-venir", 90)}
${trajet("alerte-depassee", 100)}
</div>`,
};

export const Plaque = {
  args: { immatriculation: "AA-111-AA" },
  argTypes: { immatriculation: { name: "Immatriculation", control: "text" } },
  render: ({ immatriculation }) => `${plaque(immatriculation)} <a href="/vehicules/1">${plaque(immatriculation)}</a>`,
};

// La macro `vide` : le A-route en pâle, la phrase, l'action quand l'utilisateur peut agir.
export const EcranVide = {
  name: "Écran vide",
  args: { texte: "Aucun véhicule enregistré.", action: "Nouveau véhicule" },
  argTypes: {
    texte: { name: "Texte", control: "text" },
    action: { name: "Action (vide : lecteur)", control: "text" },
  },
  render: ({ texte, action }) => ecranVide(texte, { lien: "/vehicules/nouveau", action }),
};
