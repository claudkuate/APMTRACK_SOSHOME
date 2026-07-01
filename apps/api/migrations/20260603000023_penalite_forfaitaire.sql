-- Pénalité forfaitaire par infraction (fiscalité communale).
-- Chaque commune peut délibérer une pénalité en montant fixe FCFA plutôt qu'en taux
-- (ex. DLA 3 : 4 000 F, YDE 2 : 2 500 F pour la même infraction). Si `penalite_fcfa > 0`,
-- ce forfait remplace le taux (`taux_penalite_basis_points` / `taux_penalite`) dans le
-- calcul de la pénalité de retard. NULL ou 0 = pénalité au taux, comme avant.
-- Copiée du référentiel vers `pv_interventions` au moment de la création du PV
-- (snapshot), comme les autres règles financières.

ALTER TABLE interventions
    ADD COLUMN IF NOT EXISTS penalite_fcfa BIGINT
        CHECK (penalite_fcfa IS NULL OR penalite_fcfa >= 0);

ALTER TABLE pv_interventions
    ADD COLUMN IF NOT EXISTS penalite_fcfa BIGINT
        CHECK (penalite_fcfa IS NULL OR penalite_fcfa >= 0);
