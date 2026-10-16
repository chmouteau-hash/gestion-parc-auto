-- Les documents de l'achat du véhicule (#109) : quatre types de plus. La liste des types est
-- répétée dans src/commun/types.rs (TypeDocument) ; le dossier d'un document (Achat,
-- Administratif, Entretien, Autres) se déduit de son type dans le code et ne se stocke pas.
-- Rien n'est effacé ni converti : les documents existants gardent leur type.
ALTER TABLE documents
    DROP CONSTRAINT documents_type_document_check,
    ADD CONSTRAINT documents_type_document_check CHECK (type_document IN
        ('photo_vehicule', 'facture', 'controle_technique', 'carte_grise', 'assurance',
         'facture_achat', 'certificat_cession', 'contrat_achat', 'garantie', 'permis', 'autre'));
