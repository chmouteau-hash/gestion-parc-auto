-- Un véhicule peut avoir plusieurs conducteurs en même temps (véhicule partagé).
--
-- Le schéma initial limitait un véhicule à une seule affectation en cours, par un index unique
-- partiel. Cette règle est levée : l'index est retiré.
DROP INDEX affectations_une_seule_active_par_vehicule;

-- Reste interdit : le même conducteur affecté deux fois en cours au même véhicule. Ce n'est
-- jamais voulu (double clic, double saisie) et l'index le garantit même si deux requêtes
-- arrivent en parallèle.
CREATE UNIQUE INDEX affectations_pas_de_doublon_en_cours
    ON affectations (vehicule_id, conducteur_id)
    WHERE date_fin IS NULL;
