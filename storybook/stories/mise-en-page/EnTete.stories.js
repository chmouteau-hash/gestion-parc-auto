// L'en-tête de toutes les pages (`templates/base.html`) : menu, page en cours, compte connecté.
import { PROFIL, SUR_TELEPHONE, enTete } from "../fragments.js";

export default {
  title: "Mise en page/En-tête",
  parameters: { layout: "fullscreen" },
  args: { page: "vehicules", admin: true, login: "gestionnaire", ...PROFIL.args },
  argTypes: {
    page: {
      name: "Page en cours",
      control: "select",
      options: ["alertes", "vehicules", "conducteurs", "affectations", "interventions", "echeances", "documents"],
    },
    admin: { name: "Administrateur", control: "boolean" },
    login: { name: "Identifiant", control: "text" },
    ...PROFIL.argTypes,
  },
  render: enTete,
};

export const Administrateur = {};

// Le lecteur n'a pas l'entrée « Déposer un document ».
export const Lecteur = { args: { admin: false, login: "stagiaire" } };

// Le profil « particulier » n'a pas l'entrée « Affectations ».
export const Particulier = { name: "Profil particulier", args: { profil: "particulier" } };

export const SurTelephone = { name: "Sur téléphone", globals: SUR_TELEPHONE };
