-- Un contrôle technique a un résultat : favorable, défavorable (contre-visite à faire dans les
-- deux mois) ou critique (circulation interdite, contre-visite dans les deux mois). La contre-visite
-- est un type d'intervention à part, avec le même résultat. Le résultat reste vide pour les autres
-- interventions (contrainte), et pour les contrôles saisis avant cette migration : rien n'est
-- inventé sur l'existant. Issue #107, bloc 1.
ALTER TABLE interventions
    DROP CONSTRAINT interventions_type_intervention_check,
    ADD CONSTRAINT interventions_type_intervention_check CHECK (type_intervention IN
        ('revision', 'reparation', 'pneumatiques', 'controle_technique', 'contre_visite',
         'carrosserie', 'autre')),
    ADD COLUMN resultat_controle TEXT NOT NULL DEFAULT ''
        CHECK (resultat_controle IN ('', 'favorable', 'defavorable', 'critique')),
    ADD CONSTRAINT interventions_resultat_d_un_controle
        CHECK (resultat_controle = '' OR type_intervention IN ('controle_technique', 'contre_visite'));
