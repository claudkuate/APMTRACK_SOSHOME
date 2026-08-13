-- Source de vérité UNIQUE des montants dus par PV : base, pénalité, total.
--
-- Auparavant la règle était calculée en Rust (payments.rs::payment_computation_from_rows)
-- et n'était appliquée qu'au moment de l'encaissement ; les agrégats du tableau de bord
-- sommaient `pvs.amount_initial_fcfa`, colonne qui ne contient QUE la base et ne peut donc
-- structurellement pas refléter une pénalité. Résultat : « 160 000 en attente » au lieu de
-- 162 000, et un receveur capable d'encaisser un PV échu sans sa pénalité.
--
-- Cette vue remplace le calcul Rust. dashboard.rs, payments.rs, exports.rs et le suivi
-- public du QR code la lisent tous, de sorte que le montant affiché au receveur, le montant
-- exigé à la validation et l'agrégat du tableau de bord ne peuvent plus diverger d'un franc.
--
-- Règles reproduites, dans l'ordre :
--   * lignes retenues : pv_interventions.sujet_paiement = TRUE ;
--   * échéance par ligne = pvs.created_at + COALESCE(delai_paiement_jours, 30) jours ;
--     échéance du PV = MIN des échéances de lignes ;
--   * taux = taux_penalite_basis_points/100 s'il est renseigné (MÊME à 0), sinon
--     taux_penalite, sinon 0 — le COALESCE reproduit le `.map().or_else()` Rust, qui ne
--     se replie que sur None et jamais sur une valeur nulle explicite ;
--   * pénalité par ligne : 0 avant l'échéance ; sinon le forfait penalite_fcfa s'il est
--     > 0 (fiscalité communale, prioritaire sur le taux) ; sinon round(montant * taux/100).
--
-- Repli (PV sans AUCUNE ligne payante — PV semés par `seed-demo`, ou insérés directement) :
-- la base reste `pvs.amount_initial_fcfa` comme auparavant, MAIS la pénalité n'est plus
-- forcée à zéro : les règles sont reconstituées depuis `interventions` via la colonne
-- `pvs.intervention_id`, toujours renseignée. Sans cela un PV semé échu depuis un mois
-- restait à « pénalité — » à vie (constaté sur PV-YDE1-2026-000001, échéance 06/07/2026).
--
-- ⚠️ round() est appliqué sur NUMERIC : Postgres arrondit alors à l'entier le plus loin de
-- zéro, comme f64::round en Rust. Ne JAMAIS utiliser round(double precision) ici, qui
-- arrondit au pair le plus proche (arrondi du banquier) et ferait diverger les demi-francs.
--
-- ⚠️ CREATE OR REPLACE VIEW n'autorise l'ajout de colonnes qu'EN FIN de liste et interdit
-- de changer un type ou un ordre. Toute modification structurelle ultérieure devra passer
-- par un DROP VIEW explicite dans une nouvelle migration.

-- Chemin d'accès exact de la sous-requête latérale ci-dessous.
CREATE INDEX IF NOT EXISTS idx_pv_interventions_paying
    ON pv_interventions (pv_id)
    WHERE sujet_paiement;

CREATE OR REPLACE VIEW pv_amounts_due AS
SELECT
    a.pv_id,
    a.pv_number,
    a.commune_id,
    a.status,
    a.created_at,
    a.deleted_at,
    a.paying_line_count,
    a.amount_base_fcfa,
    a.amount_penalty_fcfa,
    a.amount_base_fcfa + a.amount_penalty_fcfa            AS amount_total_fcfa,
    a.due_date,
    (a.status IN ('EN_ATTENTE_PAIEMENT', 'EN_RETARD')
        AND a.deleted_at IS NULL)                         AS is_pending,
    (a.status IN ('EN_ATTENTE_PAIEMENT', 'EN_RETARD')
        AND a.deleted_at IS NULL
        AND now() > a.due_date)                           AS is_late
FROM (
    SELECT
        p.id                                              AS pv_id,
        p.pv_number,
        p.commune_id,
        p.status,
        p.created_at,
        p.deleted_at,
        COALESCE(agg.line_count, 0)                       AS paying_line_count,
        CASE WHEN COALESCE(agg.line_count, 0) > 0
             THEN agg.amount_base_fcfa
             ELSE COALESCE(p.amount_initial_fcfa, 0)::BIGINT
        END                                               AS amount_base_fcfa,
        CASE WHEN COALESCE(agg.line_count, 0) > 0
             THEN agg.amount_penalty_fcfa
             ELSE COALESCE(fb.amount_penalty_fcfa, 0)::BIGINT
        END                                               AS amount_penalty_fcfa,
        CASE WHEN COALESCE(agg.line_count, 0) > 0
             THEN agg.due_date
             ELSE fb.due_date
        END                                               AS due_date
    FROM pvs p

    -- Cas nominal : les lignes payantes snapshotées sur le PV.
    LEFT JOIN LATERAL (
        SELECT
            COUNT(*)                                      AS line_count,
            COALESCE(SUM(line.montant_fcfa), 0)::BIGINT   AS amount_base_fcfa,
            COALESCE(SUM(line.penalty_fcfa), 0)::BIGINT   AS amount_penalty_fcfa,
            MIN(line.due_date)                            AS due_date
        FROM (
            SELECT
                COALESCE(pi.montant_fcfa, 0)::BIGINT      AS montant_fcfa,
                d.due_date,
                CASE
                    WHEN now() <= d.due_date               THEN 0::BIGINT
                    WHEN COALESCE(pi.penalite_fcfa, 0) > 0 THEN pi.penalite_fcfa
                    WHEN r.rate > 0
                        THEN round(COALESCE(pi.montant_fcfa, 0)::NUMERIC * r.rate / 100)::BIGINT
                    ELSE 0::BIGINT
                END                                       AS penalty_fcfa
            FROM pv_interventions pi
            CROSS JOIN LATERAL (
                SELECT p.created_at
                       + (COALESCE(pi.delai_paiement_jours, 30) * 86400) * INTERVAL '1 second'
                           AS due_date
            ) d
            CROSS JOIN LATERAL (
                SELECT COALESCE(
                           pi.taux_penalite_basis_points::NUMERIC / 100,
                           pi.taux_penalite::NUMERIC,
                           0::NUMERIC
                       ) AS rate
            ) r
            WHERE pi.pv_id = p.id
              AND pi.sujet_paiement = TRUE
        ) line
    ) agg ON TRUE

    -- Repli : aucune ligne payante -> règles reconstituées depuis le référentiel.
    LEFT JOIN LATERAL (
        SELECT
            fd.due_date,
            CASE
                WHEN COALESCE(p.amount_initial_fcfa, 0) <= 0 THEN 0::BIGINT
                WHEN now() <= fd.due_date                    THEN 0::BIGINT
                WHEN COALESCE(i.penalite_fcfa, 0) > 0        THEN i.penalite_fcfa
                WHEN fr.rate > 0
                    THEN round(COALESCE(p.amount_initial_fcfa, 0)::NUMERIC * fr.rate / 100)::BIGINT
                ELSE 0::BIGINT
            END AS amount_penalty_fcfa
        FROM (SELECT 1) AS _one
        LEFT JOIN interventions i ON i.id = p.intervention_id
        CROSS JOIN LATERAL (
            SELECT p.created_at
                   + (COALESCE(i.delai_paiement_jours, 30) * 86400) * INTERVAL '1 second'
                       AS due_date
        ) fd
        CROSS JOIN LATERAL (
            SELECT COALESCE(
                       i.taux_penalite_basis_points::NUMERIC / 100,
                       i.taux_penalite::NUMERIC,
                       0::NUMERIC
                   ) AS rate
        ) fr
    ) fb ON TRUE
) a;

COMMENT ON VIEW pv_amounts_due IS
    'Montants dus par PV (base / penalite / total) — source unique, cf. migration 26.';
