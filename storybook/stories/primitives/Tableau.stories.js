// `.tableau` : toutes les listes de l'application. Sous 700 px de large, chaque ligne devient une
// fiche ; `app.js` recopie alors l'intitulé de chaque colonne sur ses cellules.
import {
  CARBURANTS,
  ETATS_AFFECTATION,
  ETATS_ECHEANCE,
  STATUTS_VEHICULE,
  SUR_TELEPHONE,
  VEHICULES,
  boutonEnLigne,
  libelle,
  nombre,
  pastille,
  pastilleStatut,
  tableau,
} from "../fragments.js";

export default {
  title: "Primitives/Tableau",
};

// `templates/vehicules/liste.html`.
const vehicules = () =>
  tableau(
    ["Immatriculation", "Marque", "Modèle", "Mise en service", "Carburant", "Kilométrage", "Statut"],
    VEHICULES.map((v) => [
      `<a href="/vehicules/${v.id}">${v.immatriculation}</a>`,
      v.marque,
      v.modele,
      v.miseEnService,
      libelle(CARBURANTS, v.carburant),
      nombre(`${v.kilometrage} km`),
      pastilleStatut(STATUTS_VEHICULE, v.statut),
    ]),
  );

// `templates/echeances/liste.html` : le statut est une pastille, qui dit aussi « Dépassée » ; la
// dernière colonne, sans intitulé, porte les actions.
const echeances = () => {
  const [DEPASSEE, A_TRAITER, TRAITEE] = ETATS_ECHEANCE;
  const ligne = (id, immatriculation, type, date, etat) => [
    `<a href="/vehicules/${id}">${immatriculation}</a>`,
    type,
    date,
    pastille(etat[1], etat[0]),
    [
      `<a href="/echeances/${id}/modifier">Modifier</a>`,
      etat !== TRAITEE ? boutonEnLigne({ action: `/echeances/${id}/traiter?retour=liste`, texte: "Marquer traitée" }) : "",
      boutonEnLigne({
        action: `/echeances/${id}/supprimer`,
        texte: "Supprimer",
        danger: true,
        confirmer: "Supprimer cette échéance ?",
      }),
    ].join("\n"),
  ];
  return tableau(
    ["Véhicule", "Type", "Date limite", "Statut", ""],
    [
      ligne(1, "AB-123-CD", "Contrôle technique", "02/10/2026", DEPASSEE),
      ligne(2, "EF-456-GH", "Assurance", "31/12/2026", A_TRAITER),
      ligne(3, "JK-789-LM", "Entretien périodique", "15/03/2026", TRAITEE),
    ],
  );
};

// `templates/alertes.html` : la ligne et sa pastille portent la classe du niveau. La pastille a le
// fond de la ligne ; sa bordure et son texte la détachent.
const alertes = () => {
  const ligne = (classe, niveau, objet, motif, date, delai, lien) => ({
    classe,
    cellules: [pastille(niveau, classe), `<a href="${lien}">${objet}</a>`, motif, date, delai, `<a href="${lien}">Ouvrir la fiche</a>`],
  });
  return tableau(
    ["Niveau", "Objet", "Motif", "Date limite", "Délai", ""],
    [
      ligne("alerte-depassee", "Dépassée", "Véhicule AB-123-CD", "Contrôle technique", "02/10/2026", "depuis 10 jours", "/vehicules/1"),
      ligne("alerte-a-venir", "À venir", "Véhicule EF-456-GH", "Assurance", "12/10/2026", "aujourd'hui", "/vehicules/2"),
      ligne("alerte-a-venir", "À venir", "Conducteur Léa Martin", "Permis de conduire", "21/10/2026", "dans 9 jours", "/conducteurs/1"),
    ],
  );
};

// `templates/affectations/liste.html`, sans les actions : une affectation en cours est en vert.
const affectations = () => {
  const [enCours, , terminee] = ETATS_AFFECTATION;
  return tableau(
    ["Véhicule", "Conducteur", "Début", "Fin", "État"],
    [
      { classe: "ligne-active", cellules: ["AB-123-CD", "Léa Martin", "10/01/2026", "—", pastille(enCours[1], enCours[0])] },
      ["AB-123-CD", "Karim Benali", "15/03/2022", "10/01/2026", pastille(terminee[1], terminee[0])],
    ],
  );
};

// `templates/conducteurs/liste.html` : le service de la deuxième ligne n'est pas renseigné.
const conducteurs = () =>
  tableau(
    ["Nom", "Prénom", "Service", "Expiration du permis", "Statut"],
    [
      [`<a href="/conducteurs/1">Martin</a>`, "Léa", "Commercial", "21/10/2026", pastille("Actif", "statut-actif")],
      [`<a href="/conducteurs/2">Benali</a>`, "Karim", "", "04/06/2031", pastille("Actif", "statut-actif")],
    ],
  );

export const Liste = { render: vehicules };

export const AvecActions = { name: "Avec actions", render: echeances };

export const LignesDAlerte = { name: "Lignes d'alerte", render: alertes };

export const LigneActive = { name: "Ligne active", render: affectations };

export const ListeSurTelephone = { name: "Liste, sur téléphone", globals: SUR_TELEPHONE, render: vehicules };

export const ActionsSurTelephone = { name: "Avec actions, sur téléphone", globals: SUR_TELEPHONE, render: echeances };

export const AlertesSurTelephone = { name: "Lignes d'alerte, sur téléphone", globals: SUR_TELEPHONE, render: alertes };

// Une cellule sans contenu ne laisse pas de ligne blanche dans la fiche.
export const CelluleVideSurTelephone = { name: "Cellule vide, sur téléphone", globals: SUR_TELEPHONE, render: conducteurs };
