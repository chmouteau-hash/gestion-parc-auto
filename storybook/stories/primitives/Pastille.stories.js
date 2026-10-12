// `.pastille` : une étiquette arrondie. Sa couleur vient d'une seconde classe, bâtie sur le code
// du statut (`statut-en_service`) ou sur l'état d'une affectation (`etat-en_cours`).
import { ETATS_AFFECTATION, STATUTS_CONDUCTEUR, STATUTS_VEHICULE, pastille, pastilleStatut } from "../fragments.js";

const CLASSES = [
  "",
  ...STATUTS_VEHICULE.map(([code]) => `statut-${code}`),
  ...STATUTS_CONDUCTEUR.map(([code]) => `statut-${code}`),
  ...ETATS_AFFECTATION.map(([classe]) => classe),
];

export default {
  title: "Primitives/Pastille",
};

export const AuChoix = {
  name: "Au choix",
  args: { texte: "En service", classe: "statut-en_service" },
  argTypes: {
    texte: { name: "Texte", control: "text" },
    classe: { name: "Classe de couleur", control: "select", options: CLASSES },
  },
  render: ({ texte, classe }) => pastille(texte, classe),
};

// Sans seconde classe : c'est la pastille « Dépassée » / « À venir » de la vue Alertes, où la
// couleur est portée par la ligne du tableau.
export const Neutre = {
  render: () => `${pastille("Dépassée")} ${pastille("À venir")}`,
};

export const StatutsDeVehicule = {
  name: "Statuts d'un véhicule",
  render: () => STATUTS_VEHICULE.map(([code]) => pastilleStatut(STATUTS_VEHICULE, code)).join(" "),
};

export const StatutsDeConducteur = {
  name: "Statuts d'un conducteur",
  render: () => STATUTS_CONDUCTEUR.map(([code]) => pastilleStatut(STATUTS_CONDUCTEUR, code)).join(" "),
};

export const EtatsDAffectation = {
  name: "États d'une affectation",
  render: () => ETATS_AFFECTATION.map(([classe, texte]) => pastille(texte, classe)).join(" "),
};
