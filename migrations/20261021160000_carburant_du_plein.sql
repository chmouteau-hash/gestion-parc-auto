-- Le carburant pris à un plein (#126, suite du bloc 1, demandé le 21/10/2026) : SP95, SP95-E10,
-- SP98, E85, gazole, gazole B10, GPL, GNV, électricité, autre. C'est ce qu'on a mis ce jour-là,
-- pas ce que le véhicule accepte (`vehicules.carburant`) : du SP98 dans une essence, une recharge
-- sur un hybride rechargeable. C'est lui qui fixe l'unité de la quantité (litres, kWh, kg).
-- Vide pour un péage, obligatoire pour un plein (contrainte `depenses_carburant_du_plein`).
--
-- Les pleins déjà saisis reçoivent le carburant que l'application propose d'après le véhicule
-- (gazole pour un diesel, SP95-E10 pour une essence ou un hybride, électricité pour un
-- électrique, GPL pour un GPL, « autre » sinon) : c'est ce que le formulaire aurait pré-rempli,
-- et chacun se corrige depuis « Modifier ».

ALTER TABLE depenses
    ADD COLUMN carburant TEXT NOT NULL DEFAULT '' CHECK (carburant IN
        ('', 'sp95', 'sp95_e10', 'sp98', 'e85', 'gazole', 'gazole_b10', 'gpl', 'gnv',
         'electricite', 'autre'));

UPDATE depenses d
SET carburant = CASE v.carburant
    WHEN 'diesel' THEN 'gazole'
    WHEN 'essence' THEN 'sp95_e10'
    WHEN 'hybride' THEN 'sp95_e10'
    WHEN 'electrique' THEN 'electricite'
    WHEN 'gpl' THEN 'gpl'
    ELSE 'autre'
END
FROM vehicules v
WHERE v.id = d.vehicule_id AND d.type_depense = 'carburant' AND d.carburant = '';

ALTER TABLE depenses
    ADD CONSTRAINT depenses_carburant_du_plein
        CHECK ((type_depense = 'carburant') = (carburant <> ''));
