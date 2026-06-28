-- Un signalement est en réalité une plainte contre un Agent de Police
-- Municipale (sorte de contre-PV). On enrichit la table avec l'agent visé,
-- la date/heure de l'action contestée et le numéro de PV éventuellement lié.

ALTER TABLE signalements
    ADD COLUMN IF NOT EXISTS reported_agent_matricule TEXT,
    ADD COLUMN IF NOT EXISTS reported_agent_nom       TEXT,
    ADD COLUMN IF NOT EXISTS incident_datetime        TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS pv_number_ref            TEXT;
