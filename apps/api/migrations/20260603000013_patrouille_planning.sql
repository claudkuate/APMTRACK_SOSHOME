-- Planification des patrouilles : dates prévues (distinctes des dates réelles
-- date_debut/date_fin posées par les actions start/end).
ALTER TABLE patrouilles
    ADD COLUMN date_debut_prevue TIMESTAMPTZ,
    ADD COLUMN date_fin_prevue   TIMESTAMPTZ;
