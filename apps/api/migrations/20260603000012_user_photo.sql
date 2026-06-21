-- Photo de profil pour les comptes utilisateurs.
-- Stocke la cle objet S3/MinIO de l'avatar (alimentee par l'endpoint d'upload).
-- Les agents disposent deja d'une colonne photo_url equivalente.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS photo_url TEXT;
