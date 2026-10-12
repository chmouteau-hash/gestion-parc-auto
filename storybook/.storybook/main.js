// Storybook : le catalogue d'interface de l'application. C'est un outil de développement, rangé
// dans ce dossier avec ses dépendances Node ; l'application livrée n'en contient rien.
//
// L'application rend ses pages avec Askama, que Storybook ne sait pas exécuter. Les stories
// (`../stories/`) contiennent donc du HTML recopié des gabarits de `templates/`, habillé par la
// vraie feuille de style et animé par le vrai script (voir `preview.js`).

// Comme l'application, le catalogue ne parle qu'au poste qui le lance : Storybook ne doit rien
// demander à son éditeur. Trois appels existent, et chacun se coupe à un endroit différent :
// - les statistiques d'usage, par `disableTelemetry` ci-dessous ;
// - la recherche d'une version plus récente, faite au démarrage avant la lecture de ce fichier :
//   par `--no-version-updates` dans le script de `package.json` ;
// - la page des nouveautés, lue à chaque ouverture du catalogue : aucune option ne la coupe
//   (`disableWhatsNewNotifications` ne masque que l'annonce). Ce fichier s'exécutant dans le
//   serveur de Storybook, toute demande vers le site de l'éditeur y est refusée ici, et
//   Storybook se comporte comme hors connexion.
// `tests/catalogue.rs` vérifie que les deux premiers réglages sont toujours là.
const SITE_EDITEUR = "storybook.js.org";

function versEditeur(ressource) {
  try {
    const { hostname } = new URL(ressource?.url ?? ressource);
    return hostname === SITE_EDITEUR || hostname.endsWith(`.${SITE_EDITEUR}`);
  } catch {
    return false;
  }
}

// Storybook peut relire ce fichier en cours de route : le refus n'est posé qu'une fois.
if (!globalThis.fetch.refuseEditeur) {
  const demander = globalThis.fetch;
  globalThis.fetch = Object.assign(
    (ressource, options) =>
      versEditeur(ressource)
        ? Promise.reject(new Error(`Demande vers ${SITE_EDITEUR} refusée par .storybook/main.js`))
        : demander(ressource, options),
    { refuseEditeur: true },
  );
}

/** @type {import("@storybook/html-vite").StorybookConfig} */
const config = {
  stories: ["../stories/**/*.stories.js"],
  addons: [],
  framework: { name: "@storybook/html-vite", options: {} },
  // Le dossier `static/` de l'application, servi tel quel : `preview.js` y prend `app.js`.
  staticDirs: [{ from: "../../static", to: "/static" }],
  core: { disableTelemetry: true, disableWhatsNewNotifications: true },
  features: {
    // Le fond de page vient de la feuille de style de l'application, pas d'un réglage de Storybook.
    backgrounds: false,
    // Pas d'encart « Get started » : il invite entre autres à publier le catalogue en ligne.
    sidebarOnboardingChecklist: false,
  },
};

export default config;
