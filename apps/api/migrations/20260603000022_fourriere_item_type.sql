-- Fourrières : objets non-véhicules.
-- La mise en fourrière ne concerne plus uniquement les véhicules. `item_type` catégorise
-- l'objet retenu (véhicule, engin/matériel, marchandise saisie, animal, autre) et
-- `designation` porte un libellé générique. La plaque devient optionnelle : elle n'est
-- exigée (côté application) que pour les véhicules.

ALTER TABLE fourrieres
    ADD COLUMN IF NOT EXISTS item_type TEXT NOT NULL DEFAULT 'VEHICULE'
        CHECK (item_type IN ('VEHICULE', 'ENGIN', 'MARCHANDISE', 'ANIMAL', 'AUTRE')),
    ADD COLUMN IF NOT EXISTS designation TEXT;

ALTER TABLE fourrieres ALTER COLUMN vehicle_plate DROP NOT NULL;
