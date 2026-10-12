// `.bouton` porte l'action principale d'une page, `.bouton-discret` les actions secondaires.
// L'un comme l'autre s'écrit en <button> (envoyer un formulaire) ou en <a> (ouvrir une page).

export default {
  title: "Primitives/Bouton",
  args: { texte: "Enregistrer" },
  argTypes: { texte: { name: "Texte", control: "text" } },
};

export const Principal = {
  render: ({ texte }) => `<button type="submit" class="bouton">${texte}</button>`,
};

export const PrincipalEnLien = {
  name: "Principal, en lien",
  args: { texte: "Nouveau véhicule" },
  render: ({ texte }) => `<a class="bouton" href="/vehicules/nouveau">${texte}</a>`,
};

export const Discret = {
  args: { texte: "Annuler" },
  render: ({ texte }) => `<a href="/vehicules" class="bouton-discret">${texte}</a>`,
};

export const Danger = {
  args: { texte: "Supprimer" },
  render: ({ texte }) => `<button type="submit" class="bouton-discret danger">${texte}</button>`,
};

// Le bas de tous les formulaires de saisie.
export const ActionsDeFormulaire = {
  name: "Actions d'un formulaire",
  render: ({ texte }) => `<div class="actions">
  <button type="submit" class="bouton">${texte}</button>
  <a href="/vehicules" class="bouton-discret">Annuler</a>
</div>`,
};
