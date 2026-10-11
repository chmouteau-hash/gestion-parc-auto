-- Schéma initial de la gestion du parc automobile.
--
-- Conventions :
--   * identifiants BIGINT auto-incrémentés (GENERATED ALWAYS AS IDENTITY) ;
--   * statuts et types en TEXT avec contrainte CHECK : la liste des valeurs est ici ET dans
--     src/metier/types.rs ; une valeur qui change se change aux deux endroits ;
--   * montants en centimes entiers (BIGINT), jamais en virgule flottante ;
--   * clés étrangères avec ON DELETE explicite : supprimer un véhicule ou un conducteur emporte
--     son historique (affectations, interventions, échéances, documents). Le statut « réformé »
--     ou « inactif » est la voie normale ; la suppression est réservée aux erreurs de saisie.

-- Comptes de connexion à l'application. Ce ne sont pas les conducteurs.
CREATE TABLE utilisateurs (
    id                BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    login             TEXT NOT NULL UNIQUE,
    mot_de_passe_hash TEXT NOT NULL,
    role              TEXT NOT NULL CHECK (role IN ('administrateur', 'lecteur')),
    cree_le           TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE vehicules (
    id                   BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    -- UNIQUE crée implicitement un index : c'est lui qui sert aux recherches par immatriculation.
    immatriculation      TEXT NOT NULL UNIQUE CHECK (immatriculation <> ''),
    marque               TEXT NOT NULL CHECK (marque <> ''),
    modele               TEXT NOT NULL CHECK (modele <> ''),
    date_mise_en_service DATE NOT NULL,
    carburant            TEXT NOT NULL
        CHECK (carburant IN ('essence', 'diesel', 'electrique', 'hybride', 'gpl', 'autre')),
    kilometrage          INTEGER NOT NULL DEFAULT 0 CHECK (kilometrage >= 0),
    statut               TEXT NOT NULL DEFAULT 'en_service'
        CHECK (statut IN ('en_service', 'en_maintenance', 'reforme')),
    cree_le              TIMESTAMPTZ NOT NULL DEFAULT now(),
    modifie_le           TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Données personnelles : minimisation volontaire (nom, prénom, service, permis, statut ; rien
-- d'autre). Voir docs/exploitation.md, section RGPD.
CREATE TABLE conducteurs (
    id                     BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    nom                    TEXT NOT NULL CHECK (nom <> ''),
    prenom                 TEXT NOT NULL CHECK (prenom <> ''),
    service                TEXT NOT NULL DEFAULT '',
    date_expiration_permis DATE NOT NULL,
    statut                 TEXT NOT NULL DEFAULT 'actif' CHECK (statut IN ('actif', 'inactif')),
    cree_le                TIMESTAMPTZ NOT NULL DEFAULT now(),
    modifie_le             TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- La vue alertes filtre sur cette date.
CREATE INDEX conducteurs_date_expiration_permis_idx ON conducteurs (date_expiration_permis);

CREATE TABLE affectations (
    id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    vehicule_id   BIGINT NOT NULL REFERENCES vehicules (id) ON DELETE CASCADE,
    conducteur_id BIGINT NOT NULL REFERENCES conducteurs (id) ON DELETE CASCADE,
    date_debut    DATE NOT NULL,
    -- NULL = affectation en cours.
    date_fin      DATE CHECK (date_fin IS NULL OR date_fin >= date_debut),
    cree_le       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX affectations_vehicule_id_idx ON affectations (vehicule_id);
CREATE INDEX affectations_conducteur_id_idx ON affectations (conducteur_id);

-- Un véhicule ne peut avoir qu'une affectation active à la fois :
-- l'index unique partiel ci-dessous garantit la règle même si
-- deux requêtes arrivent en parallèle.
CREATE UNIQUE INDEX affectations_une_seule_active_par_vehicule
    ON affectations (vehicule_id)
    WHERE date_fin IS NULL;

CREATE TABLE interventions (
    id                BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    vehicule_id       BIGINT NOT NULL REFERENCES vehicules (id) ON DELETE CASCADE,
    type_intervention TEXT NOT NULL CHECK (type_intervention IN
        ('revision', 'reparation', 'pneumatiques', 'controle_technique', 'carrosserie', 'autre')),
    date_intervention DATE NOT NULL,
    kilometrage       INTEGER NOT NULL CHECK (kilometrage >= 0),
    cout_centimes     BIGINT NOT NULL DEFAULT 0 CHECK (cout_centimes >= 0),
    prestataire       TEXT NOT NULL DEFAULT '',
    notes             TEXT NOT NULL DEFAULT '',
    cree_le           TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX interventions_vehicule_id_idx ON interventions (vehicule_id);
CREATE INDEX interventions_date_intervention_idx ON interventions (date_intervention);

CREATE TABLE echeances (
    id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    vehicule_id   BIGINT NOT NULL REFERENCES vehicules (id) ON DELETE CASCADE,
    type_echeance TEXT NOT NULL
        CHECK (type_echeance IN ('controle_technique', 'assurance', 'entretien_periodique')),
    date_limite   DATE NOT NULL,
    statut        TEXT NOT NULL DEFAULT 'ouverte' CHECK (statut IN ('ouverte', 'traitee')),
    cree_le       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX echeances_vehicule_id_idx ON echeances (vehicule_id);
-- Index partiel : la vue alertes ne lit que les échéances ouvertes, triées par date limite.
CREATE INDEX echeances_ouvertes_date_limite_idx ON echeances (date_limite) WHERE statut = 'ouverte';

-- Un document est rattaché soit à un véhicule, soit à un conducteur, jamais aux deux ni à aucun.
-- Seul le chemin du fichier est en base ; le contenu vit dans DOCUMENTS_DIR.
CREATE TABLE documents (
    id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    vehicule_id   BIGINT REFERENCES vehicules (id) ON DELETE CASCADE,
    conducteur_id BIGINT REFERENCES conducteurs (id) ON DELETE CASCADE,
    nom_original  TEXT NOT NULL CHECK (nom_original <> ''),
    chemin        TEXT NOT NULL UNIQUE,
    type_mime     TEXT NOT NULL DEFAULT 'application/octet-stream',
    taille_octets BIGINT NOT NULL CHECK (taille_octets >= 0),
    cree_le       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT documents_un_seul_rattachement
        CHECK ((vehicule_id IS NULL) <> (conducteur_id IS NULL))
);

CREATE INDEX documents_vehicule_id_idx ON documents (vehicule_id);
CREATE INDEX documents_conducteur_id_idx ON documents (conducteur_id);
