-- L'historique du compteur (#94, bloc 4 ; retour #4) : les relevés du compteur faits à la main
-- (« Relever », kilométrage saisi à la création du véhicule). Avec les interventions et les pleins,
-- qui portent déjà leur kilométrage, ils forment l'historique du compteur d'un véhicule. Le
-- compteur (`vehicules.kilometrage`) devient le plus haut kilométrage de cet historique ;
-- l'application le recalcule dans la transaction de chaque écriture, et il redescend quand on
-- corrige ou supprime la saisie fautive. Les interventions et les pleins ne sont pas recopiés ici :
-- les corriger corrige l'historique.
CREATE TABLE releves_kilometrage (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    vehicule_id BIGINT NOT NULL REFERENCES vehicules (id) ON DELETE CASCADE,
    date_releve DATE NOT NULL,
    kilometrage INTEGER NOT NULL CHECK (kilometrage >= 0),
    cree_le     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- L'historique et le recalcul lisent les relevés d'un véhicule.
CREATE INDEX releves_kilometrage_par_vehicule ON releves_kilometrage (vehicule_id);

-- Relevé de reprise : un compteur plus haut que tout ce que disent les interventions et les pleins
-- du véhicule vient d'une saisie à la main ; il devient un relevé, daté de la dernière modification
-- du véhicule. Les autres compteurs se déduisent déjà de l'historique. Aucune ligne existante
-- n'est modifiée.
INSERT INTO releves_kilometrage (vehicule_id, date_releve, kilometrage)
SELECT v.id, v.modifie_le::date, v.kilometrage
FROM vehicules v
WHERE v.kilometrage > GREATEST(
    COALESCE((SELECT MAX(i.kilometrage) FROM interventions i WHERE i.vehicule_id = v.id), 0),
    COALESCE((SELECT MAX(d.kilometrage) FROM depenses d WHERE d.vehicule_id = v.id), 0)
);
