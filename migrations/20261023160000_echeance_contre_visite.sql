-- L'échéance « Contre-visite » (#107, blocs 2 et 3). Un contrôle technique défavorable la crée
-- d'office, à faire dans les deux mois ; la contre-visite enregistrée la solde. Elle n'a pas de
-- suivante : c'est le contrôle technique qui en a une, deux ans après le contrôle initial.

ALTER TABLE echeances DROP CONSTRAINT echeances_type_echeance_check;
ALTER TABLE echeances ADD CONSTRAINT echeances_type_echeance_check
    CHECK (type_echeance IN ('controle_technique', 'assurance', 'entretien_periodique', 'a_faire',
                             'contre_visite'));

-- Une seule contre-visite à faire par véhicule : elle suit le dernier contrôle défavorable. Un
-- nouveau contrôle défavorable remplace l'ancienne, que l'application passe d'abord à « traitée ».
CREATE UNIQUE INDEX echeances_une_contre_visite_ouverte
    ON echeances (vehicule_id)
    WHERE statut = 'ouverte' AND type_echeance = 'contre_visite';
