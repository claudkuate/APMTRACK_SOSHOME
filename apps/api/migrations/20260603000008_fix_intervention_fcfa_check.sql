-- Align intervention payment constraints with the FCFA integer amount contract.

ALTER TABLE interventions
    DROP CONSTRAINT IF EXISTS interventions_montant_check,
    ADD CONSTRAINT interventions_montant_check CHECK (
        sujet_paiement = FALSE
        OR (
            (montant IS NOT NULL AND montant > 0)
            OR (montant_fcfa IS NOT NULL AND montant_fcfa > 0)
        )
    );
