-- Couleur de repère du véhicule : pour reconnaître d'un coup d'œil chacun des véhicules dans
-- les listes et sur l'accueil, quand les plaques se ressemblent toutes. Le plus souvent la
-- couleur de la carrosserie, mais c'est l'utilisateur qui choisit. Facultative : vide tant
-- qu'aucune n'est choisie (la plaque garde alors sa bande bleue).
ALTER TABLE vehicules
    ADD COLUMN couleur TEXT NOT NULL DEFAULT ''
        CHECK (couleur IN ('', 'blanc', 'gris', 'noir', 'rouge', 'orange', 'jaune', 'vert',
                           'bleu', 'violet', 'marron'));
