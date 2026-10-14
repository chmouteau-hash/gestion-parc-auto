// `templates/documents/nouveau.html` : l'écran pensé pour le téléphone. Réservé à l'administrateur.
import { SUR_TELEPHONE, VEHICULES, depot, page } from "../fragments.js";

function nouveau({ vehicules = VEHICULES, erreur = "", depose = false, ...choix } = {}) {
  const suite =
    vehicules.length === 0
      ? `<p class="vide">Aucun véhicule enregistré. <a href="/vehicules/nouveau">Créer un véhicule</a> d'abord.</p>`
      : `${depot({ vehicules, ...choix })}
<p class="aide">Pour le document d'un conducteur (permis de conduire), passez par sa fiche.</p>`;
  return page({
    page: "documents",
    admin: true,
    contenu: `<h1>Déposer un document</h1>
<p class="sous-titre">Facture, procès-verbal de contrôle technique, carte grise… Choisissez le véhicule et le type, puis prenez la photo.</p>${erreur ? `\n<p class="message-erreur" role="alert">Document non déposé. ${erreur}</p>` : ""}${
      depose
        ? `\n<p class="message-succes">Enregistré : Facture pour <a href="/vehicules/1#documents">AB-123-CD (Renault Clio)</a> (<a href="/documents/1">facture-revision-2026.pdf</a>, 184 Ko).</p>`
        : ""
    }
${suite}`,
  });
}

export default {
  title: "Écrans/Déposer un document",
  parameters: { layout: "fullscreen" },
};

export const Initial = { render: () => nouveau() };

// Après un dépôt, le véhicule reste choisi pour enchaîner sur le document suivant.
export const Enregistre = {
  name: "Enregistré",
  render: () => nouveau({ depose: true, vehiculeChoisi: "1" }),
};

export const Refuse = {
  name: "Refusé",
  render: () => nouveau({ erreur: "Aucun fichier n'a été choisi.", vehiculeChoisi: "1", typeChoisi: "facture" }),
};

export const AucunVehicule = {
  name: "Aucun véhicule",
  render: () => nouveau({ vehicules: [] }),
};

export const SurTelephone = {
  name: "Initial, sur téléphone",
  globals: SUR_TELEPHONE,
  render: () => nouveau(),
};
