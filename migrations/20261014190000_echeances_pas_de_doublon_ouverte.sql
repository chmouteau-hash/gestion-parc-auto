-- Une échéance ouverte est unique par véhicule, type et date limite.
--
-- Deux échéances ouvertes identiques sont une double saisie (double clic, « suivante » enregistrée
-- deux fois) : les alertes et la fiche du véhicule montreraient deux lignes indiscernables, et
-- une échéance ne porte aucun autre champ qui les distinguerait. Une échéance traitée n'entre pas
-- dans la règle : elle reste dans l'historique et ne bloque pas une nouvelle saisie à la même
-- date.

-- Avant de poser la règle, on retire les doublons déjà saisis, en gardant le plus ancien. Ils sont
-- identiques sur tous leurs champs métier : seuls leur numéro et leur date de saisie diffèrent.
-- Sans cela, la création de l'index échouerait et l'application ne démarrerait plus.
DELETE FROM echeances e
USING echeances plus_ancienne
WHERE e.statut = 'ouverte'
  AND plus_ancienne.statut = 'ouverte'
  AND plus_ancienne.vehicule_id = e.vehicule_id
  AND plus_ancienne.type_echeance = e.type_echeance
  AND plus_ancienne.date_limite = e.date_limite
  AND plus_ancienne.id < e.id;

-- L'index garantit la règle même si deux requêtes arrivent en même temps.
CREATE UNIQUE INDEX echeances_pas_de_doublon_ouverte
    ON echeances (vehicule_id, type_echeance, date_limite)
    WHERE statut = 'ouverte';
