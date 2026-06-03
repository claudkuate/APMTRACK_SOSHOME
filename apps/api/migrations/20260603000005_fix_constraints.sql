-- Migration corrective — intégrité référentielle et contraintes manquantes

-- C1 : cascade sur pv_status_history.pv_id
-- Les archives de statut doivent suivre le PV s'il est supprimé physiquement.
ALTER TABLE pv_status_history
    DROP CONSTRAINT IF EXISTS pv_status_history_pv_id_fkey,
    ADD CONSTRAINT pv_status_history_pv_id_fkey
        FOREIGN KEY (pv_id) REFERENCES pvs(id) ON DELETE CASCADE;

-- H2a : users.commune_id — empêcher la suppression d'une commune ayant des utilisateurs
ALTER TABLE users
    DROP CONSTRAINT IF EXISTS users_commune_id_fkey,
    ADD CONSTRAINT users_commune_id_fkey
        FOREIGN KEY (commune_id) REFERENCES communes(id) ON DELETE RESTRICT;

-- H2b : agents.user_id — si le compte user est supprimé, détacher l'agent (ne pas supprimer)
ALTER TABLE agents
    DROP CONSTRAINT IF EXISTS agents_user_id_fkey,
    ADD CONSTRAINT agents_user_id_fkey
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL;

-- H1 : supprimer la colonne category_id redondante de interventions
-- (dérivable via type_id → intervention_types.category_id)
ALTER TABLE interventions DROP COLUMN IF EXISTS category_id;
