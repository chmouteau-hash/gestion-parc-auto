-- « Ce qui a été fait » gagne les contrôles et réglages qui ne posent aucune pièce (pression des
-- pneus, niveaux, géométrie…) et quatre pièces de plus (huile de boîte, amortisseurs, échappement,
-- AdBlue) ; les interventions gagnent le type « entretien courant », pour le gonflage ou les
-- niveaux du mois, qui ne sont ni une révision ni une réparation. Rien n'est changé sur
-- l'existant : les listes ne font que s'allonger. Elles sont répétées dans src/commun/types.rs
-- (Operation, TypeIntervention). Issue #94, bloc 1.
ALTER TABLE lignes_intervention
    DROP CONSTRAINT lignes_intervention_operation_check,
    ADD CONSTRAINT lignes_intervention_operation_check CHECK (operation IN
        -- Pièces et consommables.
        ('vidange', 'filtre_huile', 'filtre_air', 'filtre_habitacle', 'filtre_carburant',
         'bougies', 'courroie_distribution', 'courroie_accessoires', 'plaquettes', 'disques',
         'liquide_frein', 'liquide_refroidissement', 'huile_boite', 'batterie', 'pneus',
         'amortisseurs', 'echappement', 'balais', 'adblue', 'climatisation',
         -- Contrôles et réglages, sans pièce le plus souvent.
         'pression_pneus', 'permutation_pneus', 'niveaux', 'geometrie', 'eclairage',
         'autre'));

ALTER TABLE interventions
    DROP CONSTRAINT interventions_type_intervention_check,
    ADD CONSTRAINT interventions_type_intervention_check CHECK (type_intervention IN
        ('revision', 'entretien_courant', 'reparation', 'pneumatiques', 'controle_technique',
         'contre_visite', 'carrosserie', 'autre'));
