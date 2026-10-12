// `.formulaire` : un cadre blanc, un champ par ligne, son intitulé au-dessus, les actions en bas.
import { SUR_TELEPHONE, TYPES_INTERVENTION, VEHICULES, designation, options } from "../fragments.js";

export default {
  title: "Primitives/Formulaire",
};

// `templates/interventions/formulaire.html` : le formulaire qui réunit le plus de sortes de champs.
const intervention = ({ erreur = "", cout = "189,90" } = {}) => `${erreur ? `<p class="message-erreur">${erreur}</p>\n` : ""}<form method="post" action="/interventions" class="formulaire">
  <label>Véhicule
    <select name="vehicule_id" required>
      <option value="">— choisir —</option>
      ${options(VEHICULES.map((v) => [String(v.id), designation(v)]), "1")}
    </select>
  </label>
  <label>Type
    <select name="type_intervention" required>
      <option value="">— choisir —</option>
      ${options(TYPES_INTERVENTION, "revision")}
    </select>
  </label>
  <label>Date
    <input type="date" name="date_intervention" value="2026-09-12" required>
  </label>
  <label>Kilométrage au moment de l'intervention
    <input type="text" inputmode="numeric" name="kilometrage" value="47850" required>
  </label>
  <label>Coût en euros <small>(ex. 149,90)</small>
    <input type="text" inputmode="decimal" name="cout" value="${cout}">
  </label>
  <label>Prestataire
    <input type="text" name="prestataire" value="Garage du Centre" maxlength="200">
  </label>
  <label>Notes
    <textarea name="notes" rows="4" maxlength="5000">Vidange, filtres, contrôle des freins.</textarea>
  </label>
  <p class="aide">Si le kilométrage saisi dépasse le kilométrage courant du véhicule, celui-ci est mis à jour.</p>
  <div class="actions">
    <button type="submit" class="bouton">Enregistrer</button>
    <a href="/interventions" class="bouton-discret">Annuler</a>
  </div>
</form>`;

// `templates/affectations/formulaire.html` : ce qui serait refusé est grisé, avec la raison.
const affectation = () => `<form method="post" action="/affectations" class="formulaire">
  <label>Véhicule
    <select name="vehicule_id" required>
      <option value="">— choisir —</option>
      <option value="1">AB-123-CD (Renault Clio)</option>
      <option value="2">EF-456-GH (Peugeot Partner)</option>
      <option value="4" disabled>NP-012-QR (Citroën Berlingo) (réformé)</option>
    </select>
  </label>
  <label>Conducteur
    <select name="conducteur_id" required>
      <option value="">— choisir —</option>
      <option value="1">Léa Martin</option>
      <option value="2" disabled>Karim Benali (permis expiré)</option>
      <option value="3" disabled>Sophie Durand (inactif)</option>
    </select>
  </label>
  <label>Date de début
    <input type="date" name="date_debut" value="2026-10-12" required>
  </label>
  <label>Date de fin <small>(le jour où l'affectation s'arrête ; vide = sans date de fin)</small>
    <input type="date" name="date_fin" value="">
  </label>
  <p class="aide">Un véhicule réformé, un conducteur inactif ou dont le permis est expiré ne peut pas être choisi : sa ligne est grisée. Un véhicule peut avoir plusieurs conducteurs en même temps.</p>
  <div class="actions">
    <button type="submit" class="bouton">Enregistrer</button>
    <a href="/affectations" class="bouton-discret">Annuler</a>
  </div>
</form>`;

export const Champs = { render: () => intervention() };

// Le serveur ré-affiche le formulaire avec la saisie et le motif du refus.
export const AvecErreur = {
  name: "Avec erreur",
  render: () =>
    intervention({
      erreur: "Le champ « coût » doit être un montant en euros, par exemple 149,90 (reçu « cent euros »).",
      cout: "cent euros",
    }),
};

export const OptionsGrisees = { name: "Options grisées", render: affectation };

export const SurTelephone = { name: "Champs, sur téléphone", globals: SUR_TELEPHONE, render: () => intervention() };
