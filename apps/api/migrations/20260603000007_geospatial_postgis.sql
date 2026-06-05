-- Phase 3 — Socle géospatial PostGIS
-- Dépendances : phase1 (communes), phase2 (zones, pvs, signalements, patrouilles)
-- SRID 4326 (WGS84) partout. Les colonnes `geom` ponctuelles sont GÉNÉRÉES depuis lat/lon
-- afin de rester toujours synchronisées sans code applicatif.

CREATE EXTENSION IF NOT EXISTS postgis;

-- ─────────────────────────────────────────────────────────────────────────────
-- PV — point géométrique dérivé de gps_latitude / gps_longitude
-- ─────────────────────────────────────────────────────────────────────────────

ALTER TABLE pvs
    ADD COLUMN IF NOT EXISTS geom geometry(Point, 4326)
        GENERATED ALWAYS AS (
            CASE
                WHEN gps_longitude IS NOT NULL AND gps_latitude IS NOT NULL
                THEN ST_SetSRID(ST_MakePoint(gps_longitude::double precision, gps_latitude::double precision), 4326)
                ELSE NULL
            END
        ) STORED;

CREATE INDEX IF NOT EXISTS idx_pvs_geom ON pvs USING GIST (geom) WHERE deleted_at IS NULL;

-- ─────────────────────────────────────────────────────────────────────────────
-- Signalements — ajout lat/lon + point dérivé
-- ─────────────────────────────────────────────────────────────────────────────

ALTER TABLE signalements
    ADD COLUMN IF NOT EXISTS gps_latitude  NUMERIC(10, 7),
    ADD COLUMN IF NOT EXISTS gps_longitude NUMERIC(10, 7);

-- Colonne générée ajoutée séparément : elle référence gps_latitude/gps_longitude.
ALTER TABLE signalements
    ADD COLUMN IF NOT EXISTS geom geometry(Point, 4326)
        GENERATED ALWAYS AS (
            CASE
                WHEN gps_longitude IS NOT NULL AND gps_latitude IS NOT NULL
                THEN ST_SetSRID(ST_MakePoint(gps_longitude::double precision, gps_latitude::double precision), 4326)
                ELSE NULL
            END
        ) STORED;

CREATE INDEX IF NOT EXISTS idx_signalements_geom ON signalements USING GIST (geom);

-- ─────────────────────────────────────────────────────────────────────────────
-- Zones — centre (point) + contour (polygone)
-- ─────────────────────────────────────────────────────────────────────────────

ALTER TABLE zones
    ADD COLUMN IF NOT EXISTS centre   geometry(Point, 4326),
    ADD COLUMN IF NOT EXISTS boundary geometry(Polygon, 4326);

CREATE INDEX IF NOT EXISTS idx_zones_boundary ON zones USING GIST (boundary) WHERE deleted_at IS NULL;

-- ─────────────────────────────────────────────────────────────────────────────
-- Communes — centre (point) + contour (multipolygone)
-- ─────────────────────────────────────────────────────────────────────────────

ALTER TABLE communes
    ADD COLUMN IF NOT EXISTS centre   geometry(Point, 4326),
    ADD COLUMN IF NOT EXISTS boundary geometry(MultiPolygon, 4326);

CREATE INDEX IF NOT EXISTS idx_communes_boundary ON communes USING GIST (boundary) WHERE deleted_at IS NULL;

-- ─────────────────────────────────────────────────────────────────────────────
-- Patrouilles — itinéraire planifié (ligne) + trace terrain (points horodatés)
-- ─────────────────────────────────────────────────────────────────────────────

ALTER TABLE patrouilles
    ADD COLUMN IF NOT EXISTS itineraire geometry(LineString, 4326);

CREATE TABLE IF NOT EXISTS patrouille_positions (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    patrouille_id UUID        NOT NULL REFERENCES patrouilles(id) ON DELETE CASCADE,
    agent_id      UUID        REFERENCES agents(id),
    geom          geometry(Point, 4326) NOT NULL,
    accuracy_m    NUMERIC(8, 2),
    recorded_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_patrouille_positions_track ON patrouille_positions (patrouille_id, recorded_at);
CREATE INDEX IF NOT EXISTS idx_patrouille_positions_geom  ON patrouille_positions USING GIST (geom);
