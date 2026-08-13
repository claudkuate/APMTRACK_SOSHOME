-- Provisionnement automatique du compte agent.
--
-- Un agent est d'office un utilisateur de l'application mobile : son compte est désormais
-- créé en même temps que sa fiche (saisie manuelle comme import CSV), avec un mot de passe
-- temporaire que l'agent doit remplacer à sa première connexion.
--
-- `must_change_password` porte cette obligation. Les comptes existants ne sont pas impactés
-- (défaut FALSE) : seuls les comptes provisionnés automatiquement démarrent à TRUE.
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS must_change_password BOOLEAN NOT NULL DEFAULT FALSE;

COMMENT ON COLUMN users.must_change_password IS
    'Force la définition d''un nouveau mot de passe à la prochaine connexion (compte provisionné).';
