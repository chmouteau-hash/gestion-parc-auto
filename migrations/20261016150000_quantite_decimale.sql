-- La quantité d'une opération accepte les décimales : une vidange se compte en litres (5,4 L),
-- la climatisation en kilos de gaz. Même convention que le prix : des centièmes entiers
-- (540 = 5,4), jamais de virgule flottante. Les quantités déjà saisies sont converties. Retour #99.
ALTER TABLE lignes_intervention
    ADD COLUMN quantite_centiemes INTEGER NOT NULL DEFAULT 100 CHECK (quantite_centiemes >= 1);
UPDATE lignes_intervention SET quantite_centiemes = quantite * 100;
ALTER TABLE lignes_intervention DROP COLUMN quantite;
