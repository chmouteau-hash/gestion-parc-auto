// Les textes qui entourent listes et formulaires : refus, confirmation, liste vide, explication.

export default {
  title: "Primitives/Messages",
  argTypes: { texte: { name: "Texte", control: "text" } },
};

export const Erreur = {
  args: { texte: "L'immatriculation « AB-123-CD » est déjà utilisée par un autre véhicule." },
  render: ({ texte }) => `<p class="message-erreur">${texte}</p>`,
};

export const Succes = {
  name: "Succès",
  render: () =>
    `<p class="message-succes">Enregistré : Facture pour <a href="/vehicules/1#documents">AB-123-CD (Renault Clio)</a> (<a href="/documents/1/voir">facture-revision-2026.pdf</a>, 184 Ko).</p>`,
};

export const Vide = {
  args: { texte: "Aucun véhicule enregistré." },
  render: ({ texte }) => `<p class="vide">${texte}</p>`,
};

export const Aide = {
  args: {
    texte: "Si le kilométrage saisi dépasse le kilométrage courant du véhicule, celui-ci est mis à jour.",
  },
  render: ({ texte }) => `<p class="aide">${texte}</p>`,
};

// Sous le titre d'une page.
export const SousTitre = {
  name: "Sous-titre",
  render: () => `<h1>Échéances</h1>
<p class="sous-titre">Contrôle technique, assurance, entretien périodique. Les échéances à traiter apparaissent en premier ; la vue <a href="/alertes">Alertes</a> ne montre que celles qui sont dépassées ou proches.</p>`,
};
