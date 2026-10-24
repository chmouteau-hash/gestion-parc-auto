-- Un compte se désactive, il ne se supprime pas (#172, bloc 2) : il ne peut plus se connecter,
-- sa session en cours s'arrête à la requête suivante, mais son identifiant reste pour
-- l'historique. Les comptes existants restent actifs.
ALTER TABLE utilisateurs
    ADD COLUMN actif BOOLEAN NOT NULL DEFAULT true;
