-- Autorise le compteur de séquence 'FOURRIERE' (numéros FOUR-{CODE}-{YEAR}-{SEQ}).
-- Le CHECK initial de document_sequences ne couvrait que PV / RECEIPT / SIGNALEMENT.

ALTER TABLE document_sequences
    DROP CONSTRAINT IF EXISTS document_sequences_kind_check,
    ADD CONSTRAINT document_sequences_kind_check
        CHECK (kind IN ('PV', 'RECEIPT', 'SIGNALEMENT', 'FOURRIERE'));
