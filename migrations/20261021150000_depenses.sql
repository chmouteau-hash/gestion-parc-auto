-- Les dépenses pour rouler (#126, bloc 1) : les pleins de carburant (ou les recharges) et les
-- tickets de péage, par véhicule. Un seul écran et une seule table, avec un type dans un
-- vocabulaire fermé, comme les interventions : d'autres types (parking, lavage) s'ajouteront en
-- remplaçant la contrainte, sans nouvelle table ni nouvel écran.
--
-- Seul un plein porte une quantité (en centièmes de litre, ou de kWh pour un véhicule
-- électrique), le kilométrage au compteur (facultatif : NULL quand on ne l'a pas regardé) et la
-- case « plein complet ». Un péage n'a que sa date, son montant et son libellé (le trajet) :
-- la contrainte `depenses_detail_du_carburant` l'impose, comme `interventions_resultat_d_un_controle`.
-- La consommation et les totaux ne sont pas stockés : ils se calculent à l'affichage (#94).

CREATE TABLE depenses (
    id                 BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    vehicule_id        BIGINT NOT NULL REFERENCES vehicules (id) ON DELETE CASCADE,
    type_depense       TEXT NOT NULL CHECK (type_depense IN ('carburant', 'peage')),
    date_depense       DATE NOT NULL,
    montant_centimes   BIGINT NOT NULL DEFAULT 0 CHECK (montant_centimes >= 0),
    -- Centièmes de litre (ou de kWh) ; 0 quand la quantité n'est pas notée.
    quantite_centiemes INTEGER NOT NULL DEFAULT 0 CHECK (quantite_centiemes >= 0),
    kilometrage        INTEGER CHECK (kilometrage >= 0),
    plein_complet      BOOLEAN NOT NULL DEFAULT FALSE,
    -- La station, ou le trajet (« Lyon – Grenoble ») ; libre, facultatif.
    libelle            TEXT NOT NULL DEFAULT '',
    notes              TEXT NOT NULL DEFAULT '',
    cree_le            TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT depenses_detail_du_carburant CHECK (
        type_depense = 'carburant'
        OR (quantite_centiemes = 0 AND kilometrage IS NULL AND NOT plein_complet)
    )
);

CREATE INDEX depenses_vehicule_id_idx ON depenses (vehicule_id);
CREATE INDEX depenses_date_depense_idx ON depenses (date_depense);
