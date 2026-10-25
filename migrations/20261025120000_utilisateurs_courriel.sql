-- Les destinataires du rappel du matin se règlent par compte, dans l'application (#182) : une
-- adresse de courriel, vide tant que la personne n'en a pas donné, et le choix de recevoir le
-- rappel. Les comptes existants restent sans adresse ; le serveur d'envoi reste dans le .env.
ALTER TABLE utilisateurs
    ADD COLUMN courriel TEXT NOT NULL DEFAULT '',
    ADD COLUMN rappels BOOLEAN NOT NULL DEFAULT false;
