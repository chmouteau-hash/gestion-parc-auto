-- Une échéance « À faire » : noter ce qu'il y a à remplacer ou à faire sur un véhicule (câble
-- d'embrayage qui accroche, balais usés, voyant de plaquettes allumé…), avec une date limite
-- choisie, pour que l'application le rappelle comme un contrôle technique. Issue #98.
--
-- Ce n'est ni une intervention (ce qui *a été fait*) ni un plan d'entretien (ce qui revient
-- périodiquement) : c'est à faire **une fois**, donc sans « suivante ». C'est une échéance d'un
-- nouveau type, qui porte en plus ce qu'il y a à faire et le constat : la fiche, la liste, les
-- alertes, l'accueil, l'impression, l'API et l'export la montrent sans rien d'autre.

ALTER TABLE echeances DROP CONSTRAINT echeances_type_echeance_check;
ALTER TABLE echeances ADD CONSTRAINT echeances_type_echeance_check
    CHECK (type_echeance IN ('controle_technique', 'assurance', 'entretien_periodique', 'a_faire'));

-- Chaîne vide plutôt que NULL, comme `lignes_intervention`. `operation` est un code du vocabulaire
-- des opérations (`Operation`) : sa valeur est vérifiée par l'application, pas ici, pour que la
-- liste n'existe pas en deux exemplaires en base.
ALTER TABLE echeances
    ADD COLUMN operation TEXT NOT NULL DEFAULT '',
    -- Ce qu'est la chose à faire quand l'opération est « autre » : « Câble d'embrayage ».
    ADD COLUMN libelle   TEXT NOT NULL DEFAULT '',
    -- Pourquoi : « câble qui accroche », « voyant allumé le 16/10 ». Libre, facultatif.
    ADD COLUMN constat   TEXT NOT NULL DEFAULT '',
    -- Une échéance « à faire » dit quoi ; les autres types ne portent ni opération ni constat.
    ADD CONSTRAINT echeances_a_faire_precise
        CHECK ((type_echeance = 'a_faire') = (operation <> '')),
    ADD CONSTRAINT echeances_constat_seulement_a_faire
        CHECK (type_echeance = 'a_faire' OR constat = ''),
    ADD CONSTRAINT echeances_autre_precisee
        CHECK (operation <> 'autre' OR libelle <> ''),
    ADD CONSTRAINT echeances_libelle_seulement_autre
        CHECK (operation = 'autre' OR libelle = '');

-- Deux « à faire » à la même date pour deux choses différentes sont deux échéances légitimes ;
-- deux fois la même chose à la même date reste une double saisie. Pour les trois anciens types,
-- operation et libelle sont vides : la règle d'avant est inchangée.
DROP INDEX echeances_pas_de_doublon_ouverte;
CREATE UNIQUE INDEX echeances_pas_de_doublon_ouverte
    ON echeances (vehicule_id, type_echeance, date_limite, operation, libelle)
    WHERE statut = 'ouverte';
