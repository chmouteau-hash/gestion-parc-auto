// `templates/erreur.html`, avec les titres et messages de `src/erreurs.rs`.
import { pageEtroite } from "../fragments.js";

export default {
  title: "Écrans/Erreur",
  parameters: { layout: "fullscreen" },
  argTypes: {
    titre: { name: "Titre", control: "text" },
    message: { name: "Message", control: "text" },
  },
  render: ({ titre, message }) =>
    pageEtroite(`  <h1>${titre}</h1>
  <p class="message-erreur" role="alert">${message}</p>
  <p><a href="/">Retour à l'accueil</a></p>`),
};

export const Introuvable = {
  args: { titre: "Introuvable", message: "Cette page ou cet élément n'existe pas." },
};

export const AccesRefuse = {
  name: "Accès refusé",
  args: { titre: "Accès refusé", message: "Cette action est réservée aux administrateurs." },
};

export const SaisieRefusee = {
  name: "Saisie refusée",
  args: { titre: "Saisie refusée", message: "Cette affectation est déjà terminée à cette date." },
};

export const ErreurTechnique = {
  name: "Erreur technique",
  args: { titre: "Erreur technique", message: "Une erreur interne s'est produite. Elle a été journalisée." },
};
