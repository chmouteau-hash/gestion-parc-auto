// `templates/plus.html` : derrière le quatrième onglet d'Autoclair, le reste du menu, la
// recherche et la déconnexion.
import { ROLE, SUR_TELEPHONE, page, recherche } from "../fragments.js";

function plus({ admin, profil = "particulier" }) {
  const entreprise = profil !== "particulier";
  return page({
    page: "plus",
    admin,
    profil,
    login: admin ? "gestionnaire" : "stagiaire",
    contenu: `<h1>Plus</h1>
${recherche({ surLaPage: true })}

<ul class="liste-simple liens">
  <li><a href="/alertes" data-ico="cloche">Alertes</a> <span class="aide">échéances dépassées ou proches, permis</span></li>
  <li><a href="/vehicules" data-ico="voiture">Tous les véhicules</a> <span class="aide">réformés compris</span></li>
  <li><a href="/conducteurs" data-ico="personnes">Conducteurs</a></li>${entreprise ? `\n  <li><a href="/affectations" data-ico="echange">Affectations</a></li>` : ""}${
      admin ? `\n  <li><a href="/documents/nouveau" data-ico="depot">Déposer un document</a> <span class="aide">prendre une photo, la ranger</span></li>` : ""
    }
</ul>

<form method="post" action="/deconnexion" class="actions">
  <span class="aide">${admin ? "gestionnaire · administrateur" : "stagiaire · lecture seule"}</span>
  <button type="submit" class="bouton-discret" data-ico="sortie">Se déconnecter</button>
</form>`,
  });
}

export default {
  title: "Écrans/Plus",
  parameters: { layout: "fullscreen" },
  ...ROLE,
};

export const Autoclair = { render: ({ admin }) => plus({ admin }) };

export const SurTelephone = { name: "Sur téléphone", globals: SUR_TELEPHONE, render: ({ admin }) => plus({ admin }) };
