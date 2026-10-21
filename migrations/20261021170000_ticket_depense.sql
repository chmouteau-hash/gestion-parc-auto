-- Le ticket d'une dépense (#126, bloc 2) : la photo ou le PDF du ticket de carburant ou de péage,
-- joint depuis le formulaire de dépense, comme la facture depuis celui de l'intervention (#100).
-- Il devient un document du véhicule (type « ticket »), relié à la dépense qu'il justifie.
-- Supprimer la dépense garde le document dans les documents du véhicule, sans dépense
-- (ON DELETE SET NULL) : une pièce comptable ne part pas avec une ligne de dépense.
-- Un document relié à une dépense est toujours un document du véhicule ; que ce soit bien une
-- dépense de ce véhicule se vérifie dans le code.
ALTER TABLE documents
    ADD COLUMN depense_id BIGINT REFERENCES depenses (id) ON DELETE SET NULL,
    ADD CONSTRAINT documents_depense_du_vehicule
        CHECK (depense_id IS NULL OR vehicule_id IS NOT NULL);
CREATE INDEX documents_depense_id_idx ON documents (depense_id)
    WHERE depense_id IS NOT NULL;

-- Un type de document de plus : le ticket. La liste des types est répétée dans
-- src/commun/types.rs (TypeDocument) ; son dossier sur la fiche (« Dépenses ») se déduit du type
-- dans le code et ne se stocke pas. Rien n'est effacé ni converti.
ALTER TABLE documents
    DROP CONSTRAINT documents_type_document_check,
    ADD CONSTRAINT documents_type_document_check CHECK (type_document IN
        ('photo_vehicule', 'facture', 'controle_technique', 'carte_grise', 'assurance',
         'facture_achat', 'certificat_cession', 'contrat_achat', 'garantie', 'ticket', 'permis',
         'autre'));
