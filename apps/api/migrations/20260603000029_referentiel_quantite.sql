-- Tarifs unitaires et journaliers du référentiel (quantité × durée).
--
-- Le référentiel communal tarife des unités, pas seulement des forfaits :
--   « Mise en fourrière Gros bétail — 10 000 par jour et par bête »
--   « Locaux insalubres (boutiques) — 5 000 F par boutique »
--   « Produit d'occupation des parkings — taux horaire 1 000 / journalier 5 000 »
-- Le modèle ne portait qu'un montant forfaitaire par ligne de PV : un PV pour douze
-- chèvres gardées trois jours facturait 10 000 F au lieu de 360 000 F. Le seul
-- contournement — répéter la même infraction sur le PV — est structurellement
-- interdit (`normalize_intervention_ids` déduplique les identifiants).
--
-- Découpage : le RÉFÉRENTIEL déclare l'unité de facturation (donnée délibérée), le
-- PV porte la quantité et la durée constatées sur le terrain (donnée d'espèce).
--   * `unite IS NULL`                 -> forfait, la quantité doit rester à 1 ;
--   * `facturation_par_jour = FALSE`  -> tarif ponctuel, la durée doit rester à 1.
-- L'API refuse toute quantité ou durée qui ne serait pas adossée au référentiel :
-- un agent ne doit pas pouvoir gonfler un forfait depuis le terrain (même principe
-- que `amount_initial_fcfa`, recopié du référentiel et non saisissable).

ALTER TABLE interventions
    ADD COLUMN IF NOT EXISTS unite TEXT,
    ADD COLUMN IF NOT EXISTS facturation_par_jour BOOLEAN NOT NULL DEFAULT FALSE;

-- Convention projet : colonne de nomenclature = TEXT + CHECK (jamais un ENUM), pour
-- pouvoir étendre la liste sans migration bloquante sur une valeur en cours d'usage.
ALTER TABLE interventions
    DROP CONSTRAINT IF EXISTS interventions_unite_check,
    ADD CONSTRAINT interventions_unite_check CHECK (
        unite IS NULL
        OR unite IN ('BETE', 'UNITE', 'BOUTIQUE', 'MAISON', 'HEURE', 'JOUR', 'M2')
    );

COMMENT ON COLUMN interventions.unite IS
    'Unite de facturation delibere (NULL = forfait). La quantite est saisie au PV.';
COMMENT ON COLUMN interventions.facturation_par_jour IS
    'TRUE = montant_fcfa est un tarif JOURNALIER ; la duree est saisie au PV.';

-- Snapshot au PV : ces deux colonnes sont constatées, pas recopiées du référentiel.
-- DEFAULT 1 : les lignes existantes gardent exactement le montant déjà facturé, et
-- un client qui ignore ces champs continue de fonctionner à l'identique.
ALTER TABLE pv_interventions
    ADD COLUMN IF NOT EXISTS quantite INTEGER NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS duree_jours INTEGER NOT NULL DEFAULT 1;

ALTER TABLE pv_interventions
    DROP CONSTRAINT IF EXISTS pv_interventions_quantite_check,
    ADD CONSTRAINT pv_interventions_quantite_check CHECK (quantite > 0),
    DROP CONSTRAINT IF EXISTS pv_interventions_duree_jours_check,
    ADD CONSTRAINT pv_interventions_duree_jours_check CHECK (duree_jours > 0);

-- ─────────────────────────────────────────────────────────────────────────────
-- Vue des montants dus : le montant de ligne devient un PRODUIT.
--
-- Signature (noms, types, ordre des colonnes) STRICTEMENT inchangée, donc
-- CREATE OR REPLACE VIEW reste licite (cf. avertissement en tête de la migration 26).
-- Seules changent deux expressions internes, toutes deux dans la branche nominale :
--   * la base de ligne              -> montant_fcfa * quantite * duree_jours ;
--   * l'assiette de la pénalité au taux -> le même produit.
-- La pénalité FORFAITAIRE (`penalite_fcfa`) reste volontairement NON multipliée :
-- c'est une sanction de retard délibérée par la commune, pas un prix unitaire.
-- La branche de repli (PV sans ligne payante) est inchangée : elle repose sur
-- `pvs.amount_initial_fcfa`, qui porte déjà le total calculé à la création.
-- ─────────────────────────────────────────────────────────────────────────────

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
                -- quantite et duree_jours sont NOT NULL DEFAULT 1 : le produit vaut
                -- exactement le montant unitaire pour toute ligne antérieure.
                (COALESCE(pi.montant_fcfa, 0)
                    * pi.quantite * pi.duree_jours)::BIGINT AS montant_fcfa,
                d.due_date,
                CASE
                    WHEN now() <= d.due_date               THEN 0::BIGINT
                    WHEN COALESCE(pi.penalite_fcfa, 0) > 0 THEN pi.penalite_fcfa
                    WHEN r.rate > 0
                        THEN round((COALESCE(pi.montant_fcfa, 0)
                                * pi.quantite * pi.duree_jours)::NUMERIC
                                * r.rate / 100)::BIGINT
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
    'Montants dus par PV (base / penalite / total) — source unique, cf. migrations 26 et 29.';
