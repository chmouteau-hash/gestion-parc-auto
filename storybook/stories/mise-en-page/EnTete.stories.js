// La navigation de toutes les pages (`templates/base.html`) : la barre latérale d'Autoclair Pro,
// les quatre onglets d'Autoclair. Rendue dans une page entière pour voir la barre latérale à sa
// place (à gauche sur un grand écran, en haut et repliée sous 1 024 px).
import { PROFIL, SUR_TELEPHONE, page } from "../fragments.js";

const CONTENU = `<h1>Une page</h1>
<p class="sous-titre">Le contenu de la page vient ici.</p>`;

export default {
  title: "Mise en page/Navigation",
  parameters: { layout: "fullscreen" },
  args: { page: "vehicules", admin: true, login: "gestionnaire", ...PROFIL.args },
  argTypes: {
    page: {
      name: "Page en cours",
      control: "select",
      options: ["accueil", "alertes", "vehicules", "conducteurs", "affectations", "interventions", "echeances", "documents", "plus", "recherche"],
    },
    admin: { name: "Administrateur", control: "boolean" },
    login: { name: "Identifiant", control: "text" },
    ...PROFIL.argTypes,
  },
  render: (args) => page({ contenu: CONTENU, ...args }),
};

export const BarreLaterale = { name: "Autoclair Pro : barre latérale" };

// Le lecteur n'a pas l'entrée « Déposer un document ».
export const Lecteur = { name: "Autoclair Pro : lecteur", args: { admin: false, login: "stagiaire" } };

// Sous 1 024 px, la barre se replie derrière « Menu ».
export const ProSurTablette = { name: "Autoclair Pro : repliée (tablette)", globals: { viewport: { value: "tablette", isRotated: false } } };

export const Onglets = { name: "Autoclair : quatre onglets", args: { profil: "particulier" } };

// Sur le téléphone, les onglets passent en bas de l'écran.
export const OngletsSurTelephone = { name: "Autoclair : onglets, sur téléphone", args: { profil: "particulier" }, globals: SUR_TELEPHONE };
