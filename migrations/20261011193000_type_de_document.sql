-- Chaque document porte un type (facture, procès-verbal de contrôle technique, carte grise…),
-- choisi au dépôt, pour s'y retrouver dans la liste d'une fiche.
--
-- Les documents déjà déposés n'en ont pas : ils prennent « autre ». La liste des valeurs est
-- répétée dans src/commun/types.rs (TypeDocument).
ALTER TABLE documents
    ADD COLUMN type_document TEXT NOT NULL DEFAULT 'autre'
        CHECK (type_document IN
            ('facture', 'controle_technique', 'carte_grise', 'assurance', 'permis', 'autre'));
