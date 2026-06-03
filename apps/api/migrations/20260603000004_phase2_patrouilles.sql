-- Phase 2 — Patrouilles terrain
-- Dépendances : phase1 (communes, users, agents), phase2 (zones)

CREATE TABLE patrouilles (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    commune_id  UUID        NOT NULL REFERENCES communes(id),
    zone_id     UUID        REFERENCES zones(id),
    nom         TEXT        NOT NULL,
    description TEXT,
    status      TEXT        NOT NULL DEFAULT 'PLANIFIEE'
                    CHECK (status IN ('PLANIFIEE', 'EN_COURS', 'CLOTUREE')),
    date_debut  TIMESTAMPTZ,
    date_fin    TIMESTAMPTZ,
    created_by  UUID        NOT NULL REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMPTZ
);

CREATE TABLE patrouille_agents (
    id              UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    patrouille_id   UUID    NOT NULL REFERENCES patrouilles(id) ON DELETE CASCADE,
    agent_id        UUID    NOT NULL REFERENCES agents(id),
    role_patrouille TEXT    NOT NULL DEFAULT 'MEMBRE'
                        CHECK (role_patrouille IN ('CHEF', 'MEMBRE')),
    UNIQUE (patrouille_id, agent_id)
);

CREATE INDEX idx_patrouilles_commune  ON patrouilles (commune_id, status) WHERE deleted_at IS NULL;
CREATE INDEX idx_patrouille_agents    ON patrouille_agents (patrouille_id);
CREATE INDEX idx_patrouille_agent_id  ON patrouille_agents (agent_id);
