// `.fiche` : les caractéristiques d'un véhicule ou d'un conducteur, intitulé à gauche et valeur à
// droite. Sous 700 px, la valeur passe sous son intitulé.
import { STATUTS_CONDUCTEUR, STATUTS_VEHICULE, SUR_TELEPHONE, pastilleStatut } from "../fragments.js";

export default {
  title: "Primitives/Fiche",
};

// `templates/vehicules/fiche.html`.
const vehicule = () => `<dl class="fiche">
  <dt>Statut</dt><dd>${pastilleStatut(STATUTS_VEHICULE, "en_service")}</dd>
  <dt>Mise en service</dt><dd>15/03/2022</dd>
  <dt>Carburant</dt><dd>Essence</dd>
  <dt>Kilométrage courant</dt><dd>48200 km</dd>
  <dt>Conducteur actuel</dt>
  <dd>
    <div><a href="/conducteurs/1">Léa Martin</a> depuis le 10/01/2026</div>
    <div><a href="/affectations/nouvelle?vehicule_id=1">Affecter un autre conducteur</a></div>
  </dd>
</dl>`;

// `templates/conducteurs/fiche.html`.
const conducteur = () => `<dl class="fiche">
  <dt>Statut</dt><dd>${pastilleStatut(STATUTS_CONDUCTEUR, "inactif")}</dd>
  <dt>Service</dt><dd><em>non renseigné</em></dd>
  <dt>Expiration du permis</dt><dd>04/06/2031</dd>
</dl>`;

export const Vehicule = { name: "Véhicule", render: vehicule };

export const Conducteur = { render: conducteur };

export const SurTelephone = { name: "Véhicule, sur téléphone", globals: SUR_TELEPHONE, render: vehicule };
