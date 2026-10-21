-- Des niveaux pour les couleurs de repère les plus courantes : clair et foncé pour le gris, le
-- rouge, le vert et le bleu, clair pour le marron (un gris argent et un gris anthracite ne se
-- confondent plus), et un beige.
-- Chaque niveau est une couleur de plus, pas un second réglage : un seul code par véhicule, et
-- ceux qui existent déjà (« gris », « bleu »…) gardent leur sens, donc aucune donnée à reprendre.
--
-- La liste des codes est celle de `commun::types::Couleur` ; le test `codes_acceptes_par_la_base`
-- vérifie que la contrainte les accepte tous.

ALTER TABLE vehicules DROP CONSTRAINT vehicules_couleur_check;
ALTER TABLE vehicules ADD CONSTRAINT vehicules_couleur_check
    CHECK (couleur IN ('', 'blanc', 'beige', 'gris_clair', 'gris', 'gris_fonce', 'noir',
                       'rouge_clair', 'rouge', 'rouge_fonce', 'orange', 'jaune',
                       'vert_clair', 'vert', 'vert_fonce',
                       'bleu_clair', 'bleu', 'bleu_fonce', 'violet',
                       'marron_clair', 'marron'));
