// `templates/connexion.html` : page sans menu, puisque personne n'est encore connecté. Seul écart
// avec le gabarit : pas d'`autofocus` sur l'identifiant, qui prendrait le clavier à Storybook.
import { pageEtroite } from "../fragments.js";

const connexion = ({ erreur = "", login = "" } = {}) =>
  pageEtroite(`  <h1>Parc automobile</h1>
  <p>Connectez-vous pour accéder au suivi du parc.</p>${erreur ? `\n  <p class="message-erreur" role="alert">${erreur}</p>` : ""}
  <form method="post" action="/connexion" class="formulaire">
    <label>Identifiant
      <input type="text" name="login" value="${login}" required autocomplete="username">
    </label>
    <label>Mot de passe
      <input type="password" name="mot_de_passe" required autocomplete="current-password">
    </label>
    <div class="actions">
      <button type="submit" class="bouton">Se connecter</button>
    </div>
  </form>`);

export default {
  title: "Écrans/Connexion",
  parameters: { layout: "fullscreen" },
};

export const Initiale = { render: () => connexion() };

// L'identifiant saisi est conservé, pas le mot de passe.
export const Refusee = {
  name: "Refusée",
  render: () => connexion({ erreur: "Identifiant ou mot de passe incorrect.", login: "gestionnaire" }),
};
