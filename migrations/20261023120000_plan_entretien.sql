-- Le plan d'entretien d'un véhicule : les opérations suivies (vidange, pression des pneus…),
-- chacune avec sa périodicité, en kilomètres, en mois, ou les deux ; la première borne atteinte
-- fait foi. La dernière fois n'est pas ici : elle se calcule, c'est la plus récente intervention
-- du véhicule qui contient une ligne de l'opération. Issue #94, bloc 2.
--
-- `operation` est un code du vocabulaire des opérations (`Operation`, src/commun/types.rs), sauf
-- « autre », dont le texte libre ne permet pas de retrouver la dernière fois. Sa valeur est
-- vérifiée par l'application, pas ici, comme `echeances.operation` : la liste n'existe pas en un
-- exemplaire de plus.
CREATE TABLE plan_entretien (
    id               BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    vehicule_id      BIGINT NOT NULL REFERENCES vehicules (id) ON DELETE CASCADE,
    operation        TEXT NOT NULL CHECK (operation <> '' AND operation <> 'autre'),
    -- La périodicité ; NULL = pas de borne de ce côté. Il en faut au moins une.
    periodicite_km   INTEGER CHECK (periodicite_km > 0),
    periodicite_mois INTEGER CHECK (periodicite_mois > 0),
    cree_le          TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT plan_entretien_une_periodicite
        CHECK (periodicite_km IS NOT NULL OR periodicite_mois IS NOT NULL)
);

-- Une opération ne se suit qu'une fois par véhicule. L'index sert aussi à lire le plan d'un
-- véhicule.
CREATE UNIQUE INDEX plan_entretien_une_ligne_par_operation
    ON plan_entretien (vehicule_id, operation);
