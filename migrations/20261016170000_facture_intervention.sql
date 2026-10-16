-- La facture d'une intervention se joint depuis le formulaire d'intervention : elle devient un
-- document du véhicule (type « facture », le plus souvent), relié à l'intervention qu'elle
-- justifie. Supprimer l'intervention garde le document dans les documents du véhicule, sans
-- intervention (ON DELETE SET NULL) : une pièce comptable ne part pas avec une ligne d'entretien.
-- Un document relié à une intervention est toujours un document du véhicule ; que ce soit bien
-- une intervention de ce véhicule se vérifie dans le code. Retour #100.
ALTER TABLE documents
    ADD COLUMN intervention_id BIGINT REFERENCES interventions (id) ON DELETE SET NULL,
    ADD CONSTRAINT documents_intervention_du_vehicule
        CHECK (intervention_id IS NULL OR vehicule_id IS NOT NULL);
CREATE INDEX documents_intervention_id_idx ON documents (intervention_id)
    WHERE intervention_id IS NOT NULL;
