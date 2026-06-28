-- Géographie nationale structurée : Régions / Départements (remarques L-02, 13).
-- Les communes portaient region/departement en texte libre ; on normalise via
-- deux tables de référence seedées (10 régions, 58 départements du Cameroun) et
-- on relie les communes par clés étrangères, en conservant les colonnes texte.

CREATE TABLE IF NOT EXISTS regions (
    id UUID PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    nom TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS regions_nom_unique_ci
    ON regions (lower(nom))
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS departements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    region_id UUID NOT NULL REFERENCES regions(id),
    nom TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    UNIQUE (region_id, nom)
);

CREATE INDEX IF NOT EXISTS departements_region_idx
    ON departements (region_id)
    WHERE deleted_at IS NULL;

-- 10 régions (UUID fixes pour des FK stables) ----------------------------------
INSERT INTO regions (id, code, nom) VALUES
    ('10000000-0000-0000-0000-000000000001', 'CM-AD', 'Adamaoua'),
    ('10000000-0000-0000-0000-000000000002', 'CM-CE', 'Centre'),
    ('10000000-0000-0000-0000-000000000003', 'CM-ES', 'Est'),
    ('10000000-0000-0000-0000-000000000004', 'CM-EN', 'Extrême-Nord'),
    ('10000000-0000-0000-0000-000000000005', 'CM-LT', 'Littoral'),
    ('10000000-0000-0000-0000-000000000006', 'CM-NO', 'Nord'),
    ('10000000-0000-0000-0000-000000000007', 'CM-NW', 'Nord-Ouest'),
    ('10000000-0000-0000-0000-000000000008', 'CM-OU', 'Ouest'),
    ('10000000-0000-0000-0000-000000000009', 'CM-SU', 'Sud'),
    ('10000000-0000-0000-0000-000000000010', 'CM-SW', 'Sud-Ouest')
ON CONFLICT (code) DO NOTHING;

-- 58 départements rattachés à leur région -------------------------------------
INSERT INTO departements (region_id, nom)
SELECT r.id, d.nom
FROM regions r
JOIN (
    VALUES
        -- Adamaoua (5)
        ('Adamaoua', 'Djérem'),
        ('Adamaoua', 'Faro-et-Déo'),
        ('Adamaoua', 'Mayo-Banyo'),
        ('Adamaoua', 'Mbéré'),
        ('Adamaoua', 'Vina'),
        -- Centre (10)
        ('Centre', 'Haute-Sanaga'),
        ('Centre', 'Lekié'),
        ('Centre', 'Mbam-et-Inoubou'),
        ('Centre', 'Mbam-et-Kim'),
        ('Centre', 'Méfou-et-Afamba'),
        ('Centre', 'Méfou-et-Akono'),
        ('Centre', 'Mfoundi'),
        ('Centre', 'Nyong-et-Kéllé'),
        ('Centre', 'Nyong-et-Mfoumou'),
        ('Centre', 'Nyong-et-So''o'),
        -- Est (4)
        ('Est', 'Boumba-et-Ngoko'),
        ('Est', 'Haut-Nyong'),
        ('Est', 'Kadey'),
        ('Est', 'Lom-et-Djérem'),
        -- Extrême-Nord (6)
        ('Extrême-Nord', 'Diamaré'),
        ('Extrême-Nord', 'Logone-et-Chari'),
        ('Extrême-Nord', 'Mayo-Danay'),
        ('Extrême-Nord', 'Mayo-Kani'),
        ('Extrême-Nord', 'Mayo-Sava'),
        ('Extrême-Nord', 'Mayo-Tsanaga'),
        -- Littoral (4)
        ('Littoral', 'Moungo'),
        ('Littoral', 'Nkam'),
        ('Littoral', 'Sanaga-Maritime'),
        ('Littoral', 'Wouri'),
        -- Nord (4)
        ('Nord', 'Bénoué'),
        ('Nord', 'Faro'),
        ('Nord', 'Mayo-Louti'),
        ('Nord', 'Mayo-Rey'),
        -- Nord-Ouest (7)
        ('Nord-Ouest', 'Boyo'),
        ('Nord-Ouest', 'Bui'),
        ('Nord-Ouest', 'Donga-Mantung'),
        ('Nord-Ouest', 'Menchum'),
        ('Nord-Ouest', 'Mezam'),
        ('Nord-Ouest', 'Momo'),
        ('Nord-Ouest', 'Ngo-Ketunjia'),
        -- Ouest (8)
        ('Ouest', 'Bamboutos'),
        ('Ouest', 'Haut-Nkam'),
        ('Ouest', 'Hauts-Plateaux'),
        ('Ouest', 'Koung-Khi'),
        ('Ouest', 'Menoua'),
        ('Ouest', 'Mifi'),
        ('Ouest', 'Ndé'),
        ('Ouest', 'Noun'),
        -- Sud (4)
        ('Sud', 'Dja-et-Lobo'),
        ('Sud', 'Mvila'),
        ('Sud', 'Océan'),
        ('Sud', 'Vallée-du-Ntem'),
        -- Sud-Ouest (6)
        ('Sud-Ouest', 'Fako'),
        ('Sud-Ouest', 'Koupé-Manengouba'),
        ('Sud-Ouest', 'Lebialem'),
        ('Sud-Ouest', 'Manyu'),
        ('Sud-Ouest', 'Meme'),
        ('Sud-Ouest', 'Ndian')
) AS d(region_nom, nom) ON d.region_nom = r.nom
ON CONFLICT (region_id, nom) DO NOTHING;

-- Liaison des communes ---------------------------------------------------------
ALTER TABLE communes
    ADD COLUMN IF NOT EXISTS region_id UUID REFERENCES regions(id),
    ADD COLUMN IF NOT EXISTS departement_id UUID REFERENCES departements(id);

CREATE INDEX IF NOT EXISTS communes_region_id_idx
    ON communes (region_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS communes_departement_id_idx
    ON communes (departement_id)
    WHERE deleted_at IS NULL;

-- Réconciliation bidirectionnelle texte <-> identifiants : garantit que toute
-- commune insérée (API, seed démo, SQL direct) reste cohérente sans dupliquer
-- la logique côté applicatif.
CREATE OR REPLACE FUNCTION communes_link_geography() RETURNS trigger AS $$
BEGIN
    IF NEW.region_id IS NULL AND NEW.region IS NOT NULL THEN
        SELECT id INTO NEW.region_id
        FROM regions
        WHERE lower(nom) = lower(trim(NEW.region)) AND deleted_at IS NULL
        LIMIT 1;
    END IF;

    IF NEW.departement_id IS NULL AND NEW.departement IS NOT NULL THEN
        SELECT d.id INTO NEW.departement_id
        FROM departements d
        WHERE lower(d.nom) = lower(trim(NEW.departement))
          AND (NEW.region_id IS NULL OR d.region_id = NEW.region_id)
          AND d.deleted_at IS NULL
        LIMIT 1;
    END IF;

    IF (NEW.region IS NULL OR btrim(NEW.region) = '') AND NEW.region_id IS NOT NULL THEN
        SELECT nom INTO NEW.region FROM regions WHERE id = NEW.region_id;
    END IF;

    IF (NEW.departement IS NULL OR btrim(NEW.departement) = '') AND NEW.departement_id IS NOT NULL THEN
        SELECT nom INTO NEW.departement FROM departements WHERE id = NEW.departement_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS communes_link_geography_trg ON communes;
CREATE TRIGGER communes_link_geography_trg
    BEFORE INSERT OR UPDATE OF region, departement, region_id, departement_id ON communes
    FOR EACH ROW EXECUTE FUNCTION communes_link_geography();

-- Backfill des communes existantes --------------------------------------------
UPDATE communes c
SET region_id = r.id
FROM regions r
WHERE c.region_id IS NULL
  AND c.region IS NOT NULL
  AND lower(trim(c.region)) = lower(r.nom);

UPDATE communes c
SET departement_id = d.id
FROM departements d
WHERE c.departement_id IS NULL
  AND c.departement IS NOT NULL
  AND lower(trim(c.departement)) = lower(d.nom)
  AND (c.region_id IS NULL OR d.region_id = c.region_id);
