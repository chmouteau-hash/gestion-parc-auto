-- Ce qui a été fait pendant une intervention : une ligne par opération (vidange, filtre à huile,
-- bougies…), avec, pour mémoire, la pièce posée, sa quantité et son prix. Le coût total de
-- l'intervention reste sur l'intervention : c'est celui de la facture, main-d'œuvre comprise.
-- Les lignes partent avec leur intervention (ON DELETE CASCADE). Issue #94, bloc 1.
CREATE TABLE lignes_intervention (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    intervention_id BIGINT NOT NULL REFERENCES interventions (id) ON DELETE CASCADE,
    operation       TEXT NOT NULL CHECK (operation IN
        ('vidange', 'filtre_huile', 'filtre_air', 'filtre_habitacle', 'filtre_carburant',
         'bougies', 'courroie_distribution', 'courroie_accessoires', 'plaquettes', 'disques',
         'liquide_frein', 'liquide_refroidissement', 'batterie', 'pneus', 'balais',
         'climatisation', 'autre')),
    -- Ce qu'est l'opération quand elle est « autre » ; vide sinon.
    libelle         TEXT NOT NULL DEFAULT '',
    reference_piece TEXT NOT NULL DEFAULT '',
    quantite        INTEGER NOT NULL DEFAULT 1 CHECK (quantite >= 1),
    prix_centimes   BIGINT NOT NULL DEFAULT 0 CHECK (prix_centimes >= 0),
    CONSTRAINT lignes_intervention_autre_precisee CHECK (operation <> 'autre' OR libelle <> '')
);

CREATE INDEX lignes_intervention_intervention_id_idx ON lignes_intervention (intervention_id);

-- Une opération n'apparaît qu'une fois par intervention (la quantité dit « quatre pneus ») ;
-- seule « autre » peut se répéter, puisque son libellé la distingue.
CREATE UNIQUE INDEX lignes_intervention_pas_de_doublon
    ON lignes_intervention (intervention_id, operation) WHERE operation <> 'autre';
