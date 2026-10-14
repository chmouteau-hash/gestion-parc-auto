// `templates/documents/voir.html` : la page d'un document. Une image s'y affiche et s'imprime ;
// un PDF s'ouvre dans un autre onglet ; tout autre fichier se télécharge.
import { DOCUMENTS, PROFIL, SUR_TELEPHONE, TYPES_DOCUMENT, VEHICULES, boutonImprimer, designation, libelle, page } from "../fragments.js";

// Une « photo » fictive, dessinée : un rectangle gris clair au format d'une carte grise.
const IMAGE_FICTIVE =
  "data:image/svg+xml," +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 420"><rect width="600" height="420" fill="#e8ecf2"/><rect x="40" y="40" width="520" height="340" rx="8" fill="#f7f8fa" stroke="#c9d1dc" stroke-width="3"/><text x="300" y="220" text-anchor="middle" font-family="sans-serif" font-size="28" fill="#5b6b7f">photo fictive du document</text></svg>`,
  );

function document({ profil, document, vehicule = VEHICULES[0] }) {
  const type = libelle(TYPES_DOCUMENT, document.type);
  const nom = designation(vehicule);
  const estImage = document.nom.endsWith(".jpg");
  const estPdf = document.nom.endsWith(".pdf");
  const actions = [
    estImage ? boutonImprimer(true) : "",
    estPdf ? `<a class="bouton" href="/documents/${document.id}/voir" target="_blank" rel="noopener">Ouvrir le PDF</a>` : "",
    `<a class="bouton-discret" href="/documents/${document.id}/telecharger">Télécharger</a>`,
  ]
    .filter(Boolean)
    .join("\n    ");
  let corps;
  if (estImage) {
    corps = `<figure class="visionneuse">
  <a href="/documents/${document.id}/voir"><img src="${IMAGE_FICTIVE}" alt="${type}, ${nom}"></a>
  <figcaption>${type} · ${nom} · déposé le ${document.creeLe}</figcaption>
</figure>
<p class="aide">Un clic sur l'image l'ouvre en plein écran.</p>`;
  } else if (estPdf) {
    corps = `<p class="aide">Le PDF s'ouvre dans un nouvel onglet ; le lecteur du navigateur permet de l'imprimer.</p>`;
  } else {
    corps = `<p class="vide">Ce type de fichier ne s'affiche pas dans le navigateur : téléchargez-le pour l'ouvrir.</p>`;
  }
  return page({
    page: "vehicules",
    admin: true,
    profil,
    contenu: `<a class="retour" href="/vehicules/${vehicule.id}#documents">${vehicule.immatriculation}</a>
<div class="barre-titre">
  <h1>${type} <small>${nom}</small></h1>
  <div class="actions">
    ${actions}
  </div>
</div>
<p class="sous-titre">${document.nom} · ${document.taille} · déposé le ${document.creeLe}</p>

${corps}`,
  });
}

export default {
  title: "Écrans/Document",
  parameters: { layout: "fullscreen" },
  ...PROFIL,
};

export const Image = { render: ({ profil }) => document({ profil, document: DOCUMENTS[1] }) };

export const Pdf = { name: "PDF", render: ({ profil }) => document({ profil, document: DOCUMENTS[2] }) };

export const AutreFichier = {
  name: "Autre fichier",
  render: ({ profil }) => document({ profil, document: { id: 4, type: "autre", nom: "devis.docx", taille: "48 Ko", creeLe: "05/10/2026 08:12" } }),
};

export const SurTelephone = {
  name: "Image, sur téléphone",
  globals: SUR_TELEPHONE,
  render: ({ profil }) => document({ profil, document: DOCUMENTS[1] }),
};
