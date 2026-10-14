// `templates/vehicules/formulaire.html`, pour la création comme pour la modification. Les autres
// formulaires de saisie (conducteur, intervention, échéance, affectation) ont la même forme.
import { CARBURANTS, PROFIL, STATUTS_VEHICULE, SUR_TELEPHONE, estEntreprise, options, page } from "../fragments.js";

const VIDE = { immatriculation: "", marque: "", modele: "", dateMiseEnService: "", carburant: "", kilometrage: "", statut: "en_service" };
const CLIO = { immatriculation: "AB-123-CD", marque: "Renault", modele: "Clio", dateMiseEnService: "2022-03-15", carburant: "essence", kilometrage: "48200", statut: "en_service" };

function formulaire({ titre, action, saisie, erreur = "", profil }) {
  // La phrase d'aide parle des affectations : le profil « particulier » ne l'affiche pas.
  const aide = estEntreprise(profil)
    ? `
  <p class="aide">Passer un véhicule à « Réformé » met fin à ses affectations en cours, à la date du jour. Il ne pourra plus être affecté.</p>`
    : "";
  return page({
    page: "vehicules",
    admin: true,
    profil,
    contenu: `<a class="retour" href="/vehicules">Véhicules</a>
<h1>${titre}</h1>${erreur ? `\n<p class="message-erreur" role="alert">${erreur}</p>` : ""}
<form method="post" action="${action}" class="formulaire">
  <label>Immatriculation
    <input type="text" name="immatriculation" value="${saisie.immatriculation}" required maxlength="200">
  </label>
  <label>Marque
    <input type="text" name="marque" value="${saisie.marque}" required maxlength="200">
  </label>
  <label>Modèle
    <input type="text" name="modele" value="${saisie.modele}" required maxlength="200">
  </label>
  <label>Date de mise en service
    <input type="date" name="date_mise_en_service" value="${saisie.dateMiseEnService}" required>
  </label>
  <label>Carburant
    <select name="carburant" required>
      <option value="">— choisir —</option>
      ${options(CARBURANTS, saisie.carburant)}
    </select>
  </label>
  <label>Kilométrage courant
    <input type="text" inputmode="numeric" name="kilometrage" value="${saisie.kilometrage}" required>
  </label>
  <label>Statut
    <select name="statut" required>
      ${options(STATUTS_VEHICULE, saisie.statut)}
    </select>
  </label>${aide}
  <div class="actions">
    <button type="submit" class="bouton">Enregistrer</button>
    <a href="/vehicules" class="bouton-discret">Annuler</a>
  </div>
</form>`,
  });
}

export default {
  title: "Écrans/Véhicules/Formulaire",
  parameters: { layout: "fullscreen" },
  ...PROFIL,
};

export const Nouveau = {
  render: ({ profil }) => formulaire({ titre: "Nouveau véhicule", action: "/vehicules", saisie: VIDE, profil }),
};

export const Modification = {
  render: ({ profil }) => formulaire({ titre: "Modifier le véhicule AB-123-CD", action: "/vehicules/1", saisie: CLIO, profil }),
};

// Le serveur ré-affiche la saisie avec le motif du refus.
export const Refuse = {
  name: "Refusé",
  render: ({ profil }) =>
    formulaire({
      titre: "Nouveau véhicule",
      action: "/vehicules",
      saisie: CLIO,
      erreur: "L'immatriculation « AB-123-CD » est déjà utilisée par un autre véhicule.",
      profil,
    }),
};

export const SurTelephone = {
  name: "Modification, sur téléphone",
  globals: SUR_TELEPHONE,
  render: ({ profil }) => formulaire({ titre: "Modifier le véhicule AB-123-CD", action: "/vehicules/1", saisie: CLIO, profil }),
};
