-- La photo du véhicule : un type de document de plus (« photo_vehicule »), et pour chaque document
-- le nom de sa vignette sur le disque, calculée par le serveur au dépôt (vide quand il n'y en a
-- pas : tout document qui n'est pas une photo du véhicule). La liste des types est répétée dans
-- src/commun/types.rs (TypeDocument).
ALTER TABLE documents
    DROP CONSTRAINT documents_type_document_check,
    ADD CONSTRAINT documents_type_document_check CHECK (type_document IN
        ('photo_vehicule', 'facture', 'controle_technique', 'carte_grise', 'assurance', 'permis',
         'autre')),
    ADD COLUMN vignette TEXT NOT NULL DEFAULT '';
