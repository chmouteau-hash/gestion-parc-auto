-- Le choix de la photo qui figure en tête de la fiche d'un véhicule et sur sa carte d'accueil (#66).
-- Le choix est un marqueur sur le document : au plus une photo par véhicule, et seulement une photo
-- du véhicule. Sans marqueur, c'est la plus récente qui sert, comme avant. Supprimer la photo
-- choisie supprime son marqueur : la plus récente reprend.
ALTER TABLE documents
    ADD COLUMN photo_de_tete BOOLEAN NOT NULL DEFAULT FALSE,
    ADD CONSTRAINT documents_photo_de_tete_check
        CHECK (NOT photo_de_tete OR (type_document = 'photo_vehicule' AND vignette <> ''));

CREATE UNIQUE INDEX documents_une_photo_de_tete_par_vehicule
    ON documents (vehicule_id) WHERE photo_de_tete;
