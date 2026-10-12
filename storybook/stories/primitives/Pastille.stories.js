// `.pastille` : une étiquette arrondie. Son état (fond, bordure, texte) vient d'une seconde classe,
// bâtie sur le code du statut (`statut-en_service`), sur l'état d'une affectation
// (`etat-en_cours`) ou sur le niveau d'une alerte (`alerte-depassee`).
import { ETATS_AFFECTATION, STATUTS_CONDUCTEUR, STATUTS_VEHICULE, pastille, pastilleStatut } from "../fragments.js";

// Les deux niveaux de la vue Alertes : `Niveau::classe()` et `Niveau::libelle()`
// (`src/metier/alertes.rs`).
const NIVEAUX_ALERTE = [
  ["alerte-depassee", "Dépassée"],
  ["alerte-a-venir", "À venir"],
];

const CLASSES = [
  "",
  ...STATUTS_VEHICULE.map(([code]) => `statut-${code}`),
  ...STATUTS_CONDUCTEUR.map(([code]) => `statut-${code}`),
  ...ETATS_AFFECTATION.map(([classe]) => classe),
  ...NIVEAUX_ALERTE.map(([classe]) => classe),
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

// Sans seconde classe, la pastille prend l'état neutre. Aucun gabarit ne l'emploie ainsi : une
// pastille dit toujours son état.
export const SansClasse = {
  name: "Sans classe d'état",
  render: () => pastille("Libellé"),
};

// `templates/alertes.html` : la pastille porte la classe de la ligne. Vue seule, sur le fond blanc
// d'un tableau ; sur sa ligne teintée, voir Tableau → « Lignes d'alerte ».
export const NiveauxDAlerte = {
  name: "Niveaux d'alerte",
  render: () => NIVEAUX_ALERTE.map(([classe, texte]) => pastille(texte, classe)).join(" "),
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
