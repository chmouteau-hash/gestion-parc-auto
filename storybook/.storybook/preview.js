// Contexte commun à toutes les stories : les jetons de la charte, la feuille de style et le
// script de l'application, ceux-là mêmes que le serveur sert sous `/static/`. Les modifier se
// voit ici sans recopie. Les jetons passent avant la feuille de style, qui les emploie.
import { action } from "storybook/actions";

import "../../design-system/autoclair/code/tokens.css";
import "../../static/style.css";

// `app.js` cherche ses cibles (tableaux, formulaires, champs de fichier) une fois, au chargement
// de la page. Storybook, lui, remplace le contenu à chaque story sans recharger. Chaque extrait
// se termine donc par la balise de `templates/base.html` : Storybook exécute les scripts qu'il
// trouve dans un extrait, et `app.js` repasse ainsi sur chaque affichage.
const SCRIPT_APPLICATION = '<script src="static/app.js"></script>';

// Une primitive se montre là où l'application la pose toujours : dans la zone principale de
// `templates/base.html` (`<main class="contenu">`), qui donne aussi les marges de la page. Des
// règles de la feuille de style ne valent que là, comme les 44 px des boutons au doigt sous
// 700 px : hors de cette zone, une story montrait des boutons de 36 px que l'application
// n'affiche pas (#24). Les écrans et la mise en page, eux, bâtissent leur page entière (`page()`).
const dansLaPage = (story, { title }) => (title.startsWith("Primitives/") ? `<main class="contenu">${story()}</main>` : story());

// Rien ne part vers le serveur depuis le catalogue : suivre un lien ou envoyer un formulaire
// ferait quitter la story. L'onglet « Actions » montre ce qui serait parti. Ces écouteurs sont
// posés sur le document : ceux d'`app.js`, posés sur le formulaire, passent avant, et la
// demande de confirmation d'une suppression s'affiche comme dans l'application.
document.addEventListener("click", (evenement) => {
  const lien = evenement.target.closest("a[href]");
  if (lien) {
    evenement.preventDefault();
    action("lien suivi")(lien.getAttribute("href"));
  }
});
document.addEventListener("submit", (evenement) => {
  if (!evenement.defaultPrevented) {
    const formulaire = evenement.target;
    action("formulaire envoyé")(`${formulaire.method.toUpperCase()} ${formulaire.getAttribute("action")}`);
  }
  evenement.preventDefault();
});

/** @type {import("@storybook/html-vite").Preview} */
const preview = {
  decorators: [dansLaPage, (story) => story() + SCRIPT_APPLICATION],
  parameters: {
    // Les marges viennent de la page (`.contenu`, ou celles de l'écran), pas de Storybook.
    layout: "fullscreen",
    options: {
      storySort: {
        order: [
          "Primitives",
          ["Bouton", "Pastille", "Repères", "Messages", "Barre de titre", "Tableau", "Fiche", "Formulaire", "Dépôt de document"],
          "Mise en page",
          "Écrans",
          ["Connexion", "Accueil", "Alertes", "Véhicules", ["Liste", "Fiche", "Formulaire"], "Affectations", "Déposer un document", "Document", "Plus", "Recherche", "Erreur"],
        ],
      },
    },
    // Largeurs utiles au projet. Sous 700 px, la feuille de style présente chaque ligne de
    // tableau comme une fiche.
    viewport: {
      options: {
        telephone: { name: "Téléphone (390 px)", styles: { width: "390px", height: "844px" }, type: "mobile" },
        seuil: { name: "Sous le seuil (699 px)", styles: { width: "699px", height: "900px" }, type: "tablet" },
        tablette: { name: "Tablette (820 px)", styles: { width: "820px", height: "1180px" }, type: "tablet" },
        bureau: { name: "Bureau (1280 px)", styles: { width: "1280px", height: "800px" }, type: "desktop" },
      },
    },
  },
};

export default preview;
