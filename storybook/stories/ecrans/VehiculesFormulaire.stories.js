// `templates/vehicules/formulaire.html`, pour la création comme pour la modification. Les autres
// formulaires de saisie (conducteur, intervention, échéance, affectation) ont la même forme.
import { CARBURANTS, PROFIL, STATUTS_VEHICULE, SUR_TELEPHONE, estEntreprise, options, page } from "../fragments.js";

const VIDE = { immatriculation: "", vin: "", marque: "", modele: "", dateMiseEnService: "", carburant: "", kilometrage: "", statut: "en_service" };
// VIN fictif.
const CLIO = { immatriculation: "AB-123-CD", vin: "VF1RFB00X12345678", marque: "Renault", modele: "Clio", dateMiseEnService: "2022-03-15", carburant: "essence", kilometrage: "48200", statut: "en_service" };

function formulaire({ titre, action, saisie, erreur = "", profil, photoARechoisir = false }) {
  // La phrase d'aide parle des affectations : le profil « particulier » ne l'affiche pas.
  const aide = estEntreprise(profil)
    ? `
  <p class="aide">Passer un véhicule à « Réformé » met fin à ses affectations en cours, à la date du jour. Il ne pourra plus être affecté.</p>`
    : "";
  // Le champ photo n'existe qu'à la création ; ensuite la photo se remplace depuis la fiche.
  const creation = action === "/vehicules";
  const photo = creation
    ? `
  <p class="intitule">Photo du véhicule <small>facultatif · JPEG, PNG ou WebP, 10 Mo maximum</small></p>
  <div class="depot-boutons choix-photo">
    <label class="bouton-discret">Prendre ou choisir une photo
      <input type="file" name="photo" accept="image/*" data-taille-max="10485760" data-trop-gros="Le fichier dépasse la taille maximale de 10 Mo." data-nom-vers="photo-choisie">
    </label>
    <span class="aide" id="photo-choisie" data-vide="Aucune photo choisie.">${photoARechoisir ? "La photo choisie n'a pas été conservée : choisissez-la de nouveau." : "Aucune photo choisie."}</span>
  </div>`
    : "";
  return page({
    page: "vehicules",
    admin: true,
    profil,
    contenu: `<a class="retour" href="/vehicules">Véhicules</a>
<h1>${titre}</h1>${erreur ? `\n<p class="message-erreur" role="alert">${erreur}</p>` : ""}
<form method="post" action="${action}" class="formulaire"${creation ? ` enctype="multipart/form-data"` : ""}>
  <label>Immatriculation
    <input type="text" name="immatriculation" value="${saisie.immatriculation}" required maxlength="200">
  </label>
  <label>Numéro d'identification (VIN) <small>facultatif · case E de la carte grise, 17 caractères</small>
    <input type="text" name="vin" value="${saisie.vin}" maxlength="25" autocapitalize="characters" spellcheck="false">
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
  </label>${aide}${photo}
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

// Une photo avait été choisie : le navigateur ne la garde pas d'un envoi à l'autre, le formulaire
// le dit.
export const PhotoRefusee = {
  name: "Photo refusée",
  render: ({ profil }) =>
    formulaire({
      titre: "Nouveau véhicule",
      action: "/vehicules",
      saisie: CLIO,
      erreur: "La photo n'a pas pu être lue. Reprenez-la, ou déposez-la en JPEG.",
      profil,
      photoARechoisir: true,
    }),
};

export const SurTelephone = {
  name: "Modification, sur téléphone",
  globals: SUR_TELEPHONE,
  render: ({ profil }) => formulaire({ titre: "Modifier le véhicule AB-123-CD", action: "/vehicules/1", saisie: CLIO, profil }),
};
