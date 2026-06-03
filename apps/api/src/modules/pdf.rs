/// Génération PDF côté backend — PV et reçu de paiement.
///
/// Utilise printpdf pour produire des documents sans dépendance au rendu Angular.
use chrono::Utc;
use printpdf::*;
use sqlx::Row;
use std::io::BufWriter;

use crate::errors::ApiError;
use crate::modules::payments::PaymentResponse;
use crate::modules::pvs::PvResponse;
use sqlx::PgPool;

pub async fn generate_pv_pdf(pool: &PgPool, pv: &PvResponse) -> Result<Vec<u8>, ApiError> {
    // Charger les données complémentaires
    let commune_nom: String =
        sqlx::query_scalar("SELECT nom FROM communes WHERE id = $1")
            .bind(pv.commune_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or_else(|| "Commune".to_string());

    let agent_row = sqlx::query("SELECT matricule, full_name FROM agents WHERE id = $1")
        .bind(pv.agent_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

    let (agent_matricule, agent_nom) = agent_row
        .map(|r| {
            let m: String = r.get("matricule");
            let n: String = r.get("full_name");
            (m, n)
        })
        .unwrap_or_else(|| (String::new(), String::new()));

    let interv_nom: String =
        sqlx::query_scalar("SELECT nom FROM interventions WHERE id = $1")
            .bind(pv.intervention_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or_else(|| "Intervention".to_string());

    let bytes = build_pv_pdf(pv, &commune_nom, &agent_matricule, &agent_nom, &interv_nom)?;
    Ok(bytes)
}

pub async fn generate_receipt_pdf(
    pool: &PgPool,
    payment: &PaymentResponse,
    pv: &PvResponse,
) -> Result<Vec<u8>, ApiError> {
    let commune_nom: String =
        sqlx::query_scalar("SELECT nom FROM communes WHERE id = $1")
            .bind(payment.commune_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or_else(|| "Commune".to_string());

    let bytes = build_receipt_pdf(payment, pv, &commune_nom)?;
    Ok(bytes)
}

// ─────────────────────────────────────────────────────────────────────────────
// Builders PDF
// ─────────────────────────────────────────────────────────────────────────────

fn build_pv_pdf(
    pv: &PvResponse,
    commune_nom: &str,
    agent_matricule: &str,
    agent_nom: &str,
    interv_nom: &str,
) -> Result<Vec<u8>, ApiError> {
    let (doc, page1, layer1) =
        PdfDocument::new("Procès-Verbal", Mm(210.0), Mm(297.0), "Page 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let font = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| ApiError::internal(format!("PDF font error: {e}")))?;
    let font_regular = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| ApiError::internal(format!("PDF font error: {e}")))?;

    // En-tête
    current_layer.use_text("PROCÈS-VERBAL DE CONSTATATION", 18.0, Mm(20.0), Mm(275.0), &font);
    current_layer.use_text(
        &format!("Commune de {commune_nom}"),
        12.0,
        Mm(20.0),
        Mm(265.0),
        &font,
    );

    // Numéro PV
    current_layer.use_text(
        &format!("N° {}", pv.pv_number),
        14.0,
        Mm(20.0),
        Mm(252.0),
        &font,
    );

    let date_str = pv.created_at.format("%d/%m/%Y %H:%M").to_string();
    current_layer.use_text(
        &format!("Date : {date_str}"),
        10.0,
        Mm(20.0),
        Mm(243.0),
        &font_regular,
    );

    // Agent
    current_layer.use_text("AGENT VERBALISATEUR", 11.0, Mm(20.0), Mm(230.0), &font);
    current_layer.use_text(
        &format!("Matricule : {agent_matricule}"),
        10.0,
        Mm(20.0),
        Mm(222.0),
        &font_regular,
    );
    current_layer.use_text(
        &format!("Nom : {agent_nom}"),
        10.0,
        Mm(20.0),
        Mm(215.0),
        &font_regular,
    );

    // Verbalisé
    current_layer.use_text("PERSONNE VERBALISÉE", 11.0, Mm(20.0), Mm(202.0), &font);
    current_layer.use_text(
        &format!("Nom : {}", pv.verbalized_name.as_deref().unwrap_or("-")),
        10.0,
        Mm(20.0),
        Mm(194.0),
        &font_regular,
    );
    current_layer.use_text(
        &format!(
            "Identifiant : {}",
            pv.verbalized_identifier.as_deref().unwrap_or("-")
        ),
        10.0,
        Mm(20.0),
        Mm(187.0),
        &font_regular,
    );
    if let Some(ref plate) = pv.vehicle_plate {
        current_layer.use_text(
            &format!("Plaque : {plate}"),
            10.0,
            Mm(20.0),
            Mm(180.0),
            &font_regular,
        );
    }

    // Infraction
    current_layer.use_text("INFRACTION CONSTATÉE", 11.0, Mm(20.0), Mm(167.0), &font);
    current_layer.use_text(
        &format!("Nature : {interv_nom}"),
        10.0,
        Mm(20.0),
        Mm(159.0),
        &font_regular,
    );
    if let Some(loc) = &pv.location_description {
        current_layer.use_text(
            &format!("Lieu : {loc}"),
            10.0,
            Mm(20.0),
            Mm(152.0),
            &font_regular,
        );
    }

    // Montant
    current_layer.use_text("MONTANT DE L'AMENDE", 11.0, Mm(20.0), Mm(139.0), &font);
    let montant_str = pv
        .amount_initial
        .map(|m| format!("{:.0} FCFA", m))
        .unwrap_or_else(|| "Non applicable".to_string());
    current_layer.use_text(&montant_str, 13.0, Mm(20.0), Mm(131.0), &font);

    // Statut
    current_layer.use_text(
        &format!("Statut : {}", pv.status),
        10.0,
        Mm(20.0),
        Mm(120.0),
        &font_regular,
    );

    // Pied de page
    current_layer.use_text(
        "Document généré par le système APMTRACK",
        8.0,
        Mm(20.0),
        Mm(20.0),
        &font_regular,
    );
    current_layer.use_text(
        &format!("Imprimé le {}", Utc::now().format("%d/%m/%Y %H:%M")),
        8.0,
        Mm(130.0),
        Mm(20.0),
        &font_regular,
    );

    serialize_pdf(doc)
}

fn build_receipt_pdf(
    payment: &PaymentResponse,
    pv: &PvResponse,
    commune_nom: &str,
) -> Result<Vec<u8>, ApiError> {
    let (doc, page1, layer1) =
        PdfDocument::new("Reçu de Paiement", Mm(210.0), Mm(148.0), "Page 1");
    let current_layer = doc.get_page(page1).get_layer(layer1);

    let font = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| ApiError::internal(format!("PDF font error: {e}")))?;
    let font_regular = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| ApiError::internal(format!("PDF font error: {e}")))?;

    current_layer.use_text("REÇU DE PAIEMENT", 16.0, Mm(20.0), Mm(132.0), &font);
    current_layer.use_text(
        &format!("Commune de {commune_nom}"),
        11.0,
        Mm(20.0),
        Mm(123.0),
        &font,
    );

    if let Some(ref num) = payment.receipt_number {
        current_layer.use_text(
            &format!("Réf. reçu : {num}"),
            11.0,
            Mm(20.0),
            Mm(113.0),
            &font,
        );
    }

    current_layer.use_text(
        &format!("PV n° : {}", pv.pv_number),
        10.0,
        Mm(20.0),
        Mm(104.0),
        &font_regular,
    );

    let paid_str = payment
        .paid_at
        .map(|d| d.format("%d/%m/%Y %H:%M").to_string())
        .unwrap_or_else(|| Utc::now().format("%d/%m/%Y %H:%M").to_string());
    current_layer.use_text(
        &format!("Date paiement : {paid_str}"),
        10.0,
        Mm(20.0),
        Mm(96.0),
        &font_regular,
    );

    current_layer.use_text(
        &format!("Montant dû : {:.0} FCFA", payment.amount_due),
        10.0,
        Mm(20.0),
        Mm(84.0),
        &font_regular,
    );
    if payment.amount_penalty > 0.0 {
        current_layer.use_text(
            &format!("Pénalités : {:.0} FCFA", payment.amount_penalty),
            10.0,
            Mm(20.0),
            Mm(77.0),
            &font_regular,
        );
    }
    current_layer.use_text(
        &format!("Total dû : {:.0} FCFA", payment.amount_total),
        11.0,
        Mm(20.0),
        Mm(70.0),
        &font,
    );
    current_layer.use_text(
        &format!("Montant encaissé : {:.0} FCFA", payment.amount_paid),
        13.0,
        Mm(20.0),
        Mm(60.0),
        &font,
    );

    current_layer.use_text("PAIEMENT VALIDÉ", 14.0, Mm(20.0), Mm(48.0), &font);

    current_layer.use_text(
        "Document généré par APMTRACK",
        8.0,
        Mm(20.0),
        Mm(15.0),
        &font_regular,
    );

    serialize_pdf(doc)
}

fn serialize_pdf(doc: PdfDocumentReference) -> Result<Vec<u8>, ApiError> {
    let mut buf = BufWriter::new(Vec::new());
    doc.save(&mut buf)
        .map_err(|e| ApiError::internal(format!("PDF serialization error: {e}")))?;
    buf.into_inner()
        .map_err(|e| ApiError::internal(format!("PDF buffer error: {e}")))
}
