-- Photos preuve attachées à un PV. Les octets vivent dans l'object storage
-- (MinIO/S3) ; seule la métadonnée et la clé d'objet sont stockées en base.
CREATE TABLE pv_photos (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pv_id        UUID NOT NULL REFERENCES pvs(id) ON DELETE CASCADE,
    commune_id   UUID NOT NULL REFERENCES communes(id),
    object_key   TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size_bytes   BIGINT NOT NULL CHECK (size_bytes > 0),
    uploaded_by  UUID REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at   TIMESTAMPTZ
);

CREATE INDEX idx_pv_photos_pv ON pv_photos (pv_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_pv_photos_commune ON pv_photos (commune_id) WHERE deleted_at IS NULL;
