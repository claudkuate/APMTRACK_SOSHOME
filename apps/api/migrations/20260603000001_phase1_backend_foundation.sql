CREATE TABLE IF NOT EXISTS roles (
    id UUID PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO roles (id, code, label)
VALUES
    ('00000000-0000-0000-0000-000000000001', 'SUPER_ADMIN', 'Administrateur systeme'),
    ('00000000-0000-0000-0000-000000000002', 'ADMIN_COMMUNE', 'Administrateur communal'),
    ('00000000-0000-0000-0000-000000000003', 'APM_AGENT', 'Agent APM'),
    ('00000000-0000-0000-0000-000000000004', 'SUPERVISEUR', 'Superviseur'),
    ('00000000-0000-0000-0000-000000000005', 'RECEVEUR', 'Receveur municipal')
ON CONFLICT (code) DO NOTHING;

CREATE TABLE IF NOT EXISTS communes (
    id UUID PRIMARY KEY,
    code TEXT NOT NULL,
    nom TEXT NOT NULL,
    region TEXT NOT NULL,
    departement TEXT NOT NULL,
    adresse TEXT,
    telephone TEXT,
    email TEXT,
    site_web TEXT,
    logo_url TEXT,
    theme_color TEXT,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS communes_code_unique_ci
    ON communes (lower(code))
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS communes_active_idx
    ON communes (active)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    full_name TEXT NOT NULL,
    commune_id UUID REFERENCES communes(id),
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS users_email_unique_ci
    ON users (lower(email))
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS users_commune_id_idx
    ON users (commune_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS user_roles (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, role_id)
);

CREATE TABLE IF NOT EXISTS agents (
    id UUID PRIMARY KEY,
    matricule TEXT NOT NULL,
    full_name TEXT NOT NULL,
    commune_id UUID NOT NULL REFERENCES communes(id),
    grade TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'ACTIF',
    date_prise_fonction DATE,
    formation_nasla BOOLEAN NOT NULL DEFAULT FALSE,
    photo_url TEXT,
    telephone TEXT,
    email TEXT,
    user_id UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CONSTRAINT agents_status_check CHECK (
        status IN ('ACTIF', 'SUSPENDU', 'RETRAITE', 'INACTIF')
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS agents_matricule_unique_ci
    ON agents (lower(matricule))
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS agents_commune_id_idx
    ON agents (commune_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS refresh_tokens (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS refresh_tokens_user_id_idx
    ON refresh_tokens (user_id);

CREATE INDEX IF NOT EXISTS refresh_tokens_valid_idx
    ON refresh_tokens (expires_at)
    WHERE revoked_at IS NULL;

CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id UUID,
    old_value JSONB,
    new_value JSONB,
    ip_address TEXT,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS audit_logs_user_id_idx
    ON audit_logs (user_id);

CREATE INDEX IF NOT EXISTS audit_logs_entity_idx
    ON audit_logs (entity_type, entity_id);

CREATE INDEX IF NOT EXISTS audit_logs_created_at_idx
    ON audit_logs (created_at DESC);
