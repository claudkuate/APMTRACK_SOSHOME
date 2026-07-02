-- Réaligne les compteurs documentaires (document_sequences) sur les numéros
-- déjà présents en base. Des données semées/reprises ont pu écrire des numéros
-- au-delà du compteur : la génération serveur réémettait alors un numéro
-- existant → violation UNIQUE (23505) dont le rollback annulait aussi
-- l'incrément du compteur → toute nouvelle émission échouait en boucle
-- (constaté sur la validation des paiements : compteur RECEIPT à 2 alors que
-- REC-...-000002 était déjà semé).
-- Contrat de next_document_sequence : prochain numéro émis = next_value stocké,
-- d'où next_value = max(seq existant) + 1. GREATEST garantit l'idempotence
-- (un compteur en avance n'est jamais reculé).

INSERT INTO document_sequences (commune_id, kind, year, next_value)
SELECT commune_id, 'RECEIPT', year, MAX(seq) + 1
FROM (
    SELECT commune_id,
           ((regexp_match(receipt_number, '-(\d{4})-(\d{6})$'))[1])::int    AS year,
           ((regexp_match(receipt_number, '-(\d{4})-(\d{6})$'))[2])::bigint AS seq
    FROM payments
    WHERE receipt_number ~ '-\d{4}-\d{6}$'
) numbered
GROUP BY commune_id, year
ON CONFLICT (commune_id, kind, year) DO UPDATE SET
    next_value = GREATEST(document_sequences.next_value, EXCLUDED.next_value),
    updated_at = now();

INSERT INTO document_sequences (commune_id, kind, year, next_value)
SELECT commune_id, 'PV', year, MAX(seq) + 1
FROM (
    SELECT commune_id,
           ((regexp_match(pv_number, '-(\d{4})-(\d{6})$'))[1])::int    AS year,
           ((regexp_match(pv_number, '-(\d{4})-(\d{6})$'))[2])::bigint AS seq
    FROM pvs
    WHERE pv_number ~ '-\d{4}-\d{6}$'
) numbered
GROUP BY commune_id, year
ON CONFLICT (commune_id, kind, year) DO UPDATE SET
    next_value = GREATEST(document_sequences.next_value, EXCLUDED.next_value),
    updated_at = now();

INSERT INTO document_sequences (commune_id, kind, year, next_value)
SELECT commune_id, 'SIGNALEMENT', year, MAX(seq) + 1
FROM (
    SELECT commune_id,
           ((regexp_match(signalement_number, '-(\d{4})-(\d{6})$'))[1])::int    AS year,
           ((regexp_match(signalement_number, '-(\d{4})-(\d{6})$'))[2])::bigint AS seq
    FROM signalements
    WHERE signalement_number ~ '-\d{4}-\d{6}$'
) numbered
GROUP BY commune_id, year
ON CONFLICT (commune_id, kind, year) DO UPDATE SET
    next_value = GREATEST(document_sequences.next_value, EXCLUDED.next_value),
    updated_at = now();

INSERT INTO document_sequences (commune_id, kind, year, next_value)
SELECT commune_id, 'FOURRIERE', year, MAX(seq) + 1
FROM (
    SELECT commune_id,
           ((regexp_match(fourriere_number, '-(\d{4})-(\d{6})$'))[1])::int    AS year,
           ((regexp_match(fourriere_number, '-(\d{4})-(\d{6})$'))[2])::bigint AS seq
    FROM fourrieres
    WHERE fourriere_number ~ '-\d{4}-\d{6}$'
) numbered
GROUP BY commune_id, year
ON CONFLICT (commune_id, kind, year) DO UPDATE SET
    next_value = GREATEST(document_sequences.next_value, EXCLUDED.next_value),
    updated_at = now();
