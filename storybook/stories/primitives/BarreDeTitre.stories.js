// `.barre-titre` : un titre à gauche, ses actions à droite. Elles passent dessous quand la place
// manque.
import { SUR_TELEPHONE, boutonEnLigne } from "../fragments.js";

export default {
  title: "Primitives/Barre de titre",
};

// Haut d'une liste (`templates/vehicules/liste.html`).
const liste = () => `<div class="barre-titre">
  <h1>Véhicules</h1>
  <a class="bouton" href="/vehicules/nouveau">Nouveau véhicule</a>
</div>`;

// Haut d'une fiche (`templates/vehicules/fiche.html`) : précision en petit, puis deux actions.
const fiche = () => `<div class="barre-titre">
  <h1>AB-123-CD <small>Renault Clio</small></h1>
  <div class="actions">
    <a class="bouton" href="/vehicules/1/modifier">Modifier</a>
    ${boutonEnLigne({
      action: "/vehicules/1/supprimer",
      texte: "Supprimer",
      danger: true,
      confirmer: "Supprimer le véhicule AB-123-CD et tout son historique ? S'il a réellement existé, passez-le plutôt au statut « Réformé » (Modifier, champ Statut) : il reste dans l'historique.",
      alternative: { href: "/vehicules/1/modifier", libelle: "Modifier le statut" },
    })}
  </div>
</div>`;

// Haut d'une section de fiche.
const section = () => `<div class="barre-titre">
  <h2>Échéances</h2>
  <a class="bouton-discret" href="/echeances/nouvelle?vehicule_id=1">Ajouter une échéance</a>
</div>`;

export const DeListe = { name: "D'une liste", render: liste };

export const DeFiche = { name: "D'une fiche", render: fiche };

export const DeSection = { name: "D'une section", render: section };

export const SurTelephone = { name: "D'une fiche, sur téléphone", globals: SUR_TELEPHONE, render: fiche };
