// `.depot` : le dépôt d'un document. Le fichier part dès qu'il est choisi (`app.js`).
//
// « Prendre une photo » n'apparaît que sur un écran tactile : la feuille de style le masque
// quand l'appareil a une souris, et Storybook ne change pas cela en réduisant la largeur. Pour le
// voir, passer par le mode « appareil mobile » des outils du navigateur (F12).
import { SUR_TELEPHONE, VEHICULES, depot } from "../fragments.js";

export default {
  title: "Primitives/Dépôt de document",
};

// Page « Déposer un document » : on choisit aussi le véhicule.
export const AvecChoixDuVehicule = {
  name: "Avec choix du véhicule",
  render: () => depot({ vehicules: VEHICULES }),
};

// Sur la fiche d'un véhicule ou d'un conducteur, le rattachement est déjà connu.
export const SurUneFiche = {
  name: "Sur une fiche",
  render: () => depot({ action: "/vehicules/1/documents" }),
};

export const EnvoiEnCours = {
  name: "Envoi en cours",
  render: () => depot({ vehicules: VEHICULES, vehiculeChoisi: "1", typeChoisi: "facture", envoiEnCours: true }),
};

export const SurTelephone = {
  name: "Sur téléphone",
  globals: SUR_TELEPHONE,
  render: () => depot({ vehicules: VEHICULES }),
};
