-- Numéro d'identification du véhicule (VIN, case E de la carte grise) : 17 caractères, lettres
-- et chiffres, sans I, O ni Q (norme ISO 3779). Facultatif : vide tant qu'on ne l'a pas sous la
-- main. Unique quand il est renseigné : deux véhicules ne partagent jamais un VIN.
ALTER TABLE vehicules
    ADD COLUMN vin TEXT NOT NULL DEFAULT ''
        CHECK (vin = '' OR vin ~ '^[A-HJ-NPR-Z0-9]{17}$');

CREATE UNIQUE INDEX vehicules_vin_unique ON vehicules (vin) WHERE vin <> '';
