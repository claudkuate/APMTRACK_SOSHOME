-- Adresse e-mail du Maire (destinataire du rapport quotidien automatisé G-APM).
-- Optionnel : à défaut, le rapport est envoyé aux ADMIN_COMMUNE de la commune.

ALTER TABLE communes
    ADD COLUMN IF NOT EXISTS maire_email TEXT;
