// `templates/alertes.html` : ce qui est dépassé ou arrive dans les 30 jours, et les opérations du
// plan d'entretien à moins de 1 000 km de leur borne (#94, bloc 2b).
import { ROLE, SUR_TELEPHONE, boutonEnLigne, boutonImprimer, page, pastille, releve, tableau } from "../fragments.js";

const ALERTES = [
  { classe: "alerte-depassee", niveau: "Dépassée", objet: "Véhicule AB-123-CD", motif: "Contrôle technique", date: "02/10/2026", delai: "depuis 10 jours", lien: "/vehicules/1", echeanceId: 1 },
  { classe: "alerte-depassee", niveau: "Dépassée", objet: "Conducteur Karim Benali", motif: "Permis de conduire", date: "11/10/2026", delai: "depuis hier", lien: "/conducteurs/2" },
  { classe: "alerte-a-venir", niveau: "À venir", objet: "Véhicule EF-456-GH", motif: "Assurance", date: "12/10/2026", delai: "aujourd'hui", lien: "/vehicules/2", echeanceId: 2 },
  { classe: "alerte-a-venir", niveau: "À venir", objet: "Véhicule AB-123-CD", motif: "À faire : Câble d'embrayage", constat: "câble qui accroche", date: "20/10/2026", delai: "dans 8 jours", lien: "/vehicules/1", echeanceId: 4 },
  { classe: "alerte-a-venir", niveau: "À venir", objet: "Véhicule JK-789-LM", motif: "Entretien périodique", date: "21/10/2026", delai: "dans 9 jours", lien: "/vehicules/3", echeanceId: 3 },
  { classe: "alerte-a-venir", niveau: "À venir", objet: "Véhicule NP-321-QR", motif: "Vidange", date: "62 000 km", delai: "dans 800 km", lien: "/vehicules/4#plan", cestFait: "/interventions/nouvelle?vehicule_id=4&amp;kilometrage=61200&amp;type_intervention=revision&amp;operation=vidange" },
];

/**
 * La dernière cellule : le lien, et pour l'administrateur, « Marquer traitée » (une échéance) ou
 * « C'est fait » (une opération du plan, #94, bloc 3).
 */
function actions(alerte, admin) {
  const lien = `<a href="${alerte.lien}">Ouvrir la fiche</a>`;
  if (admin && alerte.cestFait) {
    return `${lien}\n<a class="bouton-discret" href="${alerte.cestFait}&amp;retour=alertes">C'est fait</a>`;
  }
  if (!admin || !alerte.echeanceId) {
    return lien;
  }
  const bouton = boutonEnLigne({ action: `/echeances/${alerte.echeanceId}/traiter?retour=alertes`, texte: "Marquer traitée" });
  return `${lien}\n${bouton}`;
}

// Les compteurs à relever (#94, bloc 4) : le dernier relevé, intervention ou plein a plus d'un
// mois. Une section à part, qui ne compte pas dans les dépassées ni les à venir.
const A_RELEVER = [{ id: 3, plaque: "CC-333-CC", depuis: "le 12/09/2026 · il y a 42 jours", km: "112 000 km", kmBrut: 112000 }];

function compteursARelever(aRelever, admin) {
  if (aRelever.length === 0) {
    return "";
  }
  return `
<section class="a-relever">
  <h2>Compteurs à relever</h2>
  <p class="sous-titre">Leur dernier relevé, intervention ou plein a plus d'un mois : le rythme et les estimations du plan d'entretien se fondent sur des relevés récents.</p>
  ${tableau(
    ["Véhicule", "Dernier relevé", "Compteur", ""],
    aRelever.map((v) => [`<a href="/vehicules/${v.id}">Véhicule ${v.plaque}</a>`, v.depuis, v.km, admin ? releve(v.id, v.kmBrut, { retour: "alertes" }) : ""]),
  )}
</section>`;
}

function alertes({ admin, lignes, aRelever = [] }) {
  const depassees = lignes.filter((a) => a.classe === "alerte-depassee").length;
  const liste =
    lignes.length === 0
      ? `<p class="vide">Aucune alerte : rien n'est dépassé ni prévu dans les 30 prochains jours ou les 1 000 km prochains.</p>`
      : tableau(
          ["Niveau", "Objet", "Motif", "Limite", "Délai", ""],
          lignes.map((a) => ({
            classe: a.classe,
            cellules: [pastille(a.niveau, a.classe), `<a href="${a.lien}">${a.objet}</a>`, a.constat ? `${a.motif}<br><small>${a.constat}</small>` : a.motif, a.date, a.delai, actions(a, admin)],
          })),
        );
  return page({
    page: "alertes",
    admin,
    contenu: `<div class="barre-titre">
  <h1>Alertes</h1>
  ${lignes.length === 0 ? "" : boutonImprimer()}
</div>
<p class="sous-titre">
  Échéances dépassées ou tombant dans les 30 prochains jours, et opérations du plan d'entretien dont la borne est passée ou à moins de 1 000 km ou 30 jours :
  <strong>${depassees}</strong> dépassée(s), <strong>${lignes.length - depassees}</strong> à venir.
</p>

${liste}${compteursARelever(aRelever, admin)}`,
  });
}

export default {
  title: "Écrans/Alertes",
  parameters: { layout: "fullscreen" },
  ...ROLE,
};

export const AvecAlertes = { name: "Avec alertes", render: ({ admin }) => alertes({ admin, lignes: ALERTES, aRelever: A_RELEVER }) };

export const SansAlerte = { name: "Sans alerte", render: ({ admin }) => alertes({ admin, lignes: [] }) };

export const SurTelephone = {
  name: "Avec alertes, sur téléphone",
  globals: SUR_TELEPHONE,
  render: ({ admin }) => alertes({ admin, lignes: ALERTES }),
};
