// La racine du site : `templates/accueil/particulier.html` (Autoclair) et
// `templates/accueil/entreprise.html` (Autoclair Pro). Données fictives.
import { SUR_TELEPHONE, borne, cartePhoto, ecranVide, page, pastille, plaque, releve, trajet } from "../fragments.js";

const VEHICULES = [
  { id: 2, nom: "Peugeot 208", plaque: "BB-222-BB", borne: ["borne-depassee", "5", "retard"], echeance: "Assurance le 09/10/2026", delai: "depuis 5 jours", depassee: true, presse: true, km: "31 500 km · Diesel", kmBrut: 31500, photo: true, couleur: "rouge" },
  { id: 1, nom: "Renault Clio", plaque: "AA-111-AA", borne: ["borne-a-venir", "15", "jours"], echeance: "Contrôle technique le 29/10/2026", delai: "dans 15 jours", presse: true, km: "48 200 km · Essence", kmBrut: 48200, couleur: "bleu" },
  { id: 3, nom: "Ford Transit", plaque: "CC-333-CC", borne: ["borne-a-jour", "120", "jours"], echeance: "Entretien périodique le 11/02/2027", delai: "dans 120 jours", km: "112 000 km · Diesel", kmBrut: 112000 },
];

function carte(v, admin) {
  return `  <li class="carte-vehicule">
    ${borne(...v.borne)}
    <div class="carte-corps">
      <h2><a href="/vehicules/${v.id}">${v.nom}</a> ${plaque(v.plaque, v.couleur)}</h2>
      <p>${v.depassee ? `${pastille("Dépassée", "echeance-depassee")} ` : ""}${v.echeance} <span class="delai">· ${v.delai}</span></p>
      <p class="aide">${v.km}</p>
    </div>
    ${cartePhoto(v.id, v.photo, v.couleur)}
    <div class="carte-actions">
      ${admin && v.presse ? `<form method="post" action="/echeances/${v.id}/traiter?retour=accueil" class="en-ligne"><button type="submit" class="bouton-discret">C'est fait</button></form>` : ""}
      <a class="bouton-discret" href="/vehicules/${v.id}#documents">Documents</a>
      ${admin ? releve(v.id, v.kmBrut, { retour: "accueil", enLigne: false }) : ""}
    </div>
  </li>`;
}

function particulier({ admin, vide, aJour }) {
  const bandeau = aJour || vide
    ? `<div class="message-succes bandeau" role="status">
  <strong>Tout est à jour.</strong>${vide ? "" : "<br>Prochaine échéance : Entretien périodique de Ford Transit, dans 120 jours."}
</div>`
    : `<div class="message-erreur bandeau" role="status">
  <strong>2 choses à faire</strong><br>
  Assurance · Véhicule BB-222-BB, depuis 5 jours.
  <a href="/vehicules/2">Voir</a> · <a href="/alertes">Tout voir</a>
</div>`;
  const vehicules = aJour ? VEHICULES.slice(2) : VEHICULES;
  const contenu = vide
    ? ecranVide("Aucun véhicule pour le moment.", admin ? { lien: "/vehicules/nouveau", action: "Ajouter votre premier véhicule" } : {})
    : `<ul class="cartes">
${vehicules.map((v) => carte(v, admin)).join("\n")}
</ul>
${admin ? '<div class="actions"><a class="bouton" href="/vehicules/nouveau">Ajouter un véhicule</a></div>' : ""}
<p class="aide">1 véhicule réformé, dans la <a href="/vehicules">liste des véhicules</a>.</p>`;
  return page({ page: "accueil", admin, profil: "particulier", contenu: `${bandeau}\n\n<h1>Mes véhicules</h1>\n\n${contenu}` });
}

const ALERTES = [
  ["alerte-depassee", "Dépassée", "Conducteur Lucie Petit", "/conducteurs/3", "Permis de conduire · 24/09/2026", 100, "depuis 20 jours", null],
  ["alerte-depassee", "Dépassée", "Véhicule BB-222-BB", "/vehicules/2", "Assurance · 09/10/2026", 100, "depuis 5 jours", 2],
  ["alerte-a-venir", "À venir", "Conducteur Paul Durand", "/conducteurs/2", "Permis de conduire · 26/10/2026", 60, "dans 12 jours", null],
  ["alerte-a-venir", "À venir", "Véhicule AA-111-AA", "/vehicules/1", "Contrôle technique · 29/10/2026", 50, "dans 15 jours", 1],
];

function entreprise({ admin }) {
  const lignes = ALERTES.map(
    ([classe, niveau, objet, lien, motif, dizaine, delai, echeance]) => `      <tr class="${classe}">
        <td>${pastille(niveau, classe)}</td>
        <td><a href="${lien}">${objet}</a></td>
        <td>${motif}</td>
        <td>${trajet(classe, dizaine)}</td>
        <td>${delai}</td>
        <td>${
          admin && echeance
            ? `<form method="post" action="/echeances/${echeance}/traiter?retour=accueil" class="en-ligne"><button type="submit" class="bouton-discret">Marquer traitée</button></form>`
            : `<a href="${lien}">Ouvrir la fiche</a>`
        }</td>
      </tr>`,
  );
  const contenu = `<div class="barre-titre">
  <h1>Tableau de bord</h1>
  ${admin ? '<a class="bouton" href="/vehicules/nouveau">Nouveau véhicule</a>' : ""}
</div>

<ul class="chiffres">
  <li><a href="/alertes"><strong class="chiffre-danger">2</strong><span>Échéances dépassées</span></a></li>
  <li><a href="/alertes"><strong class="chiffre-attention">2</strong><span>À venir sous 30 jours</span></a></li>
  <li><a href="/vehicules"><strong>1</strong><span>En maintenance</span></a></li>
  <li><a href="/conducteurs"><strong class="chiffre-danger">1</strong><span>Permis expiré</span></a></li>
  <li class="chiffre-accent"><a href="/vehicules"><strong>18</strong><span>En service</span></a></li>
</ul>

<section>
  <div class="barre-titre">
    <h2>Par urgence</h2>
    <a href="/alertes">Toutes les alertes</a>
  </div>
  <div class="tableau-cadre">
  <table class="tableau">
    <thead>
      <tr><th>Niveau</th><th>Objet</th><th>Motif</th><th>Trajet</th><th>Délai</th><th aria-label="Actions"></th></tr>
    </thead>
    <tbody>
${lignes.join("\n")}
    </tbody>
  </table>
  </div>
</section>

<div class="colonnes">
  <section>
    <div class="barre-titre">
      <h2>Véhicules sans conducteur</h2>
      <a href="/affectations">Affectations</a>
    </div>
    <ul class="liste-simple">
      <li><a href="/vehicules/3">${plaque("CC-333-CC")}</a> Ford Transit${admin ? ' <a class="en-ligne" href="/affectations/nouvelle?vehicule_id=3">Affecter</a>' : ""}</li>
      <li><a href="/vehicules/5">${plaque("EE-555-EE")}</a> Citroën Berlingo${admin ? ' <a class="en-ligne" href="/affectations/nouvelle?vehicule_id=5">Affecter</a>' : ""}</li>
    </ul>
  </section>
  <section>
    <div class="barre-titre">
      <h2>Dernières interventions</h2>
      <a href="/interventions">Entretien</a>
    </div>
    <ul class="liste-simple">
      <li><span class="aide">16/07/2026</span> <a href="/vehicules/1">${plaque("AA-111-AA")}</a> Révision, 45 000 km <span class="nombre-droite">149,90 €</span></li>
      <li><span class="aide">02/07/2026</span> <a href="/vehicules/3">${plaque("CC-333-CC")}</a> Pneumatiques, 111 200 km <span class="nombre-droite">412,00 €</span></li>
    </ul>
  </section>
</div>`;
  return page({ page: "accueil", admin, profil: "entreprise", contenu });
}

export default {
  title: "Écrans/Accueil",
  parameters: { layout: "fullscreen" },
  args: { admin: true },
  argTypes: { admin: { name: "Administrateur", control: "boolean" } },
};

export const Autoclair = {
  name: "Autoclair : mes véhicules",
  render: ({ admin }) => particulier({ admin }),
};

export const AutoclairSurTelephone = {
  name: "Autoclair, sur téléphone",
  globals: SUR_TELEPHONE,
  render: ({ admin }) => particulier({ admin }),
};

export const AutoclairToutAJour = {
  name: "Autoclair : tout est à jour",
  render: ({ admin }) => particulier({ admin, aJour: true }),
};

export const AutoclairVide = {
  name: "Autoclair : aucun véhicule",
  render: ({ admin }) => particulier({ admin, vide: true }),
};

// Pas de réglage « profil » ici : chaque story est l'accueil d'un profil.
export const AutoclairPro = {
  name: "Autoclair Pro : tableau de bord",
  render: ({ admin }) => entreprise({ admin }),
};
