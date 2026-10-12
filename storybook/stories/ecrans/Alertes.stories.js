// `templates/alertes.html` : la page d'accueil, ce qui est dépassé ou arrive dans les 30 jours.
import { ROLE, SUR_TELEPHONE, page, pastille, tableau } from "../fragments.js";

const ALERTES = [
  { classe: "alerte-depassee", niveau: "Dépassée", objet: "Véhicule AB-123-CD", motif: "Contrôle technique", date: "02/10/2026", delai: "depuis 10 jours", lien: "/vehicules/1" },
  { classe: "alerte-depassee", niveau: "Dépassée", objet: "Conducteur Karim Benali", motif: "Permis de conduire", date: "11/10/2026", delai: "depuis hier", lien: "/conducteurs/2" },
  { classe: "alerte-a-venir", niveau: "À venir", objet: "Véhicule EF-456-GH", motif: "Assurance", date: "12/10/2026", delai: "aujourd'hui", lien: "/vehicules/2" },
  { classe: "alerte-a-venir", niveau: "À venir", objet: "Véhicule JK-789-LM", motif: "Entretien périodique", date: "21/10/2026", delai: "dans 9 jours", lien: "/vehicules/3" },
];

function alertes({ admin, lignes }) {
  const depassees = lignes.filter((a) => a.classe === "alerte-depassee").length;
  const liste =
    lignes.length === 0
      ? `<p class="vide">Aucune alerte : rien n'est dépassé ni prévu dans les 30 prochains jours.</p>`
      : tableau(
          ["Niveau", "Objet", "Motif", "Date limite", "Délai", ""],
          lignes.map((a) => ({
            classe: a.classe,
            cellules: [pastille(a.niveau, a.classe), `<a href="${a.lien}">${a.objet}</a>`, a.motif, a.date, a.delai, `<a href="${a.lien}">Ouvrir la fiche</a>`],
          })),
        );
  return page({
    page: "alertes",
    admin,
    contenu: `<h1>Alertes</h1>
<p class="sous-titre">
  Échéances dépassées ou tombant dans les 30 prochains jours :
  <strong>${depassees}</strong> dépassée(s), <strong>${lignes.length - depassees}</strong> à venir.
</p>

${liste}`,
  });
}

export default {
  title: "Écrans/Alertes",
  parameters: { layout: "fullscreen" },
  ...ROLE,
};

export const AvecAlertes = { name: "Avec alertes", render: ({ admin }) => alertes({ admin, lignes: ALERTES }) };

export const SansAlerte = { name: "Sans alerte", render: ({ admin }) => alertes({ admin, lignes: [] }) };

export const SurTelephone = {
  name: "Avec alertes, sur téléphone",
  globals: SUR_TELEPHONE,
  render: ({ admin }) => alertes({ admin, lignes: ALERTES }),
};
