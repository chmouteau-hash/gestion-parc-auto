-- Vignette Crit'Air du véhicule (#165) : la classe de son certificat qualité de l'air, celle
-- qu'exigent les zones à faibles émissions. Choisie par l'utilisateur d'après le certificat ;
-- l'application en propose une d'après le carburant et la date de mise en service, mais ne
-- l'enregistre pas d'elle-même. Facultative : vide tant qu'aucune n'est choisie, donc vide pour
-- les véhicules déjà enregistrés. La liste est répétée dans src/commun/types.rs (CritAir).
ALTER TABLE vehicules
    ADD COLUMN crit_air TEXT NOT NULL DEFAULT ''
        CHECK (crit_air IN ('', 'e', '1', '2', '3', '4', '5', 'non_classe'));

-- Un type de document de plus : le certificat qualité de l'air, rangé dans le dossier
-- « Administratif » (le dossier se déduit du type dans le code). Rien n'est effacé ni converti.
ALTER TABLE documents
    DROP CONSTRAINT documents_type_document_check,
    ADD CONSTRAINT documents_type_document_check CHECK (type_document IN
        ('photo_vehicule', 'facture', 'controle_technique', 'carte_grise', 'assurance',
         'certificat_qualite_air', 'facture_achat', 'certificat_cession', 'contrat_achat',
         'garantie', 'ticket', 'permis', 'autre'));
