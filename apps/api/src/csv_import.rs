//! Utilitaires de lecture de CSV « terrain », partagés par les imports (agents,
//! découpage administratif).
//!
//! Les fichiers réels arrivent d'Excel en configuration française : séparateur `;`,
//! BOM UTF-8 en tête, parfois encodage ANSI (CP1252), en-têtes accentués et
//! capitalisés, colonnes optionnelles absentes, lignes vides en fin de fichier.
//! L'ancien import agents utilisait `reader.deserialize::<T>()` avec des noms d'en-tête
//! exacts, un séparateur `,` codé en dur et aucun retrait de BOM : un export Excel FR
//! échouait ligne par ligne sans message exploitable.
//!
//! `#[derive(Deserialize)]` ne peut pas répondre au besoin : `serde(alias)` est figé à
//! la compilation et `Option<T>` exige quand même que l'en-tête existe littéralement.
//! On lit donc des `StringRecord` et on construit soi-même la carte en-tête → index.

use serde::Serialize;

use crate::errors::ApiError;

/// Au-delà, le rapport d'import deviendrait illisible : on tronque la liste mais on
/// continue de compter les erreurs.
pub const MAX_REPORTED_ERRORS: usize = 200;

/// Erreur ligne à ligne, sérialisable telle quelle dans le rapport d'import.
#[derive(Debug, Serialize, Clone)]
pub struct RowError {
    pub line: usize,
    pub message: String,
}

/// Décode le corps brut d'une requête CSV.
///
/// UTF-8 en priorité ; à défaut, repli Latin-1/CP1252 (Excel FR exporte souvent en ANSI)
/// plutôt qu'un rejet sec, puis retrait d'un éventuel BOM.
pub fn decode(body: &[u8]) -> String {
    let text = match std::str::from_utf8(body) {
        Ok(value) => value.to_string(),
        Err(_) => body.iter().map(|&byte| byte as char).collect(),
    };
    text.strip_prefix('\u{feff}').unwrap_or(&text).to_string()
}

/// Déduit le séparateur depuis la première ligne non vide.
pub fn detect_delimiter(content: &str) -> u8 {
    let header = content.lines().find(|line| !line.trim().is_empty()).unwrap_or("");
    let semicolons = header.matches(';').count();
    let commas = header.matches(',').count();
    let tabs = header.matches('\t').count();
    if semicolons > commas && semicolons >= tabs {
        b';'
    } else if tabs > commas && tabs > semicolons {
        b'\t'
    } else {
        b','
    }
}

/// Normalise un en-tête : minuscules, accents retirés, tout caractère non
/// alphanumérique replié en `_` (séquences compactées).
/// `"Code_Commune_attaché"` et `"CODE COMMUNE ATTACHE"` donnent la même clé.
pub fn normalize_header(raw: &str) -> String {
    let folded: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| match ch {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .collect();

    let mut out = String::with_capacity(folded.len());
    let mut previous_underscore = true;
    for ch in folded.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            previous_underscore = false;
        } else if !previous_underscore {
            out.push('_');
            previous_underscore = true;
        }
    }
    out.trim_matches('_').to_string()
}

/// Déclaration d'une colonne attendue.
///
/// `aliases` doit contenir des clés **déjà normalisées**. `strict` distingue deux passes :
/// les alias stricts sont résolus en premier, de sorte qu'une colonne de code l'emporte
/// sur une colonne de libellé quand le fichier porte les deux (`Commune` et
/// `Code Commune` dans un export réimporté).
pub struct ColumnSpec {
    pub canonical: &'static str,
    pub aliases: &'static [&'static str],
    pub loose_aliases: &'static [&'static str],
    pub required: bool,
}

/// Carte colonne canonique → index dans l'enregistrement.
#[derive(Debug, Default, Clone)]
pub struct Columns {
    entries: Vec<(&'static str, usize)>,
}

impl Columns {
    pub fn index_of(&self, canonical: &str) -> Option<usize> {
        self.entries
            .iter()
            .find(|(name, _)| *name == canonical)
            .map(|(_, index)| *index)
    }

    /// Colonne réellement présente dans le fichier — pilote la sémantique de mise à
    /// jour (une colonne absente ne doit pas écraser la valeur existante en base).
    pub fn has(&self, canonical: &str) -> bool {
        self.index_of(canonical).is_some()
    }

    /// Valeur nettoyée, `None` si la colonne est absente, hors limites ou vide.
    pub fn get<'a>(&self, record: &'a csv::StringRecord, canonical: &str) -> Option<&'a str> {
        let value = self.index_of(canonical).and_then(|index| record.get(index))?;
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }
}

/// Résout les en-têtes du fichier contre les colonnes attendues.
///
/// Renvoie un 400 explicite listant les colonnes obligatoires manquantes **et** les
/// en-têtes réellement lus : sans cela, un fichier mal séparé produit un message
/// incompréhensible pour l'agent qui fait la reprise de données.
pub fn resolve_columns(
    headers: &csv::StringRecord,
    specs: &[ColumnSpec],
) -> Result<Columns, ApiError> {
    let normalized: Vec<String> = headers.iter().map(normalize_header).collect();
    let mut entries: Vec<(&'static str, usize)> = Vec::new();

    // Passe 1 : alias stricts.
    for spec in specs {
        if let Some(index) = normalized.iter().position(|header| {
            header == spec.canonical || spec.aliases.iter().any(|alias| alias == header)
        }) {
            entries.push((spec.canonical, index));
        }
    }
    // Passe 2 : alias larges, uniquement pour les colonnes encore non liées et sur des
    // index encore libres.
    for spec in specs {
        if entries.iter().any(|(name, _)| *name == spec.canonical) {
            continue;
        }
        if let Some(index) = normalized.iter().position(|header| {
            spec.loose_aliases.iter().any(|alias| alias == header)
                && !entries.iter().any(|(_, taken)| taken == &normalized.iter().position(|h| h == header).unwrap_or(usize::MAX))
        }) {
            if !entries.iter().any(|(_, taken)| *taken == index) {
                entries.push((spec.canonical, index));
            }
        }
    }

    let columns = Columns { entries };
    let missing: Vec<&str> = specs
        .iter()
        .filter(|spec| spec.required && !columns.has(spec.canonical))
        .map(|spec| spec.canonical)
        .collect();

    if !missing.is_empty() {
        let seen = headers.iter().collect::<Vec<_>>().join(", ");
        return Err(ApiError::bad_request(format!(
            "En-tetes CSV invalides: colonne(s) obligatoire(s) introuvable(s): {}. Colonnes lues: [{}]",
            missing.join(", "),
            seen
        )));
    }

    Ok(columns)
}

/// Lecteur tolérant : `flexible(true)` pour qu'une ligne trop courte ou trop longue ne
/// fasse pas échouer le fichier entier.
pub fn reader(content: &str, delimiter: u8) -> csv::Reader<&[u8]> {
    csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .trim(csv::Trim::All)
        .flexible(true)
        .has_headers(true)
        .from_reader(content.as_bytes())
}

/// Un enregistrement dont toutes les cellules sont vides (lignes `;;;` que produit Excel
/// en fin de fichier) ne doit pas gonfler le compteur d'ignorés.
pub fn is_blank(record: &csv::StringRecord) -> bool {
    record.iter().all(|cell| cell.trim().is_empty())
}

/// Empile une erreur en respectant le plafond d'affichage, tout en comptant le total.
pub fn push_error(
    errors: &mut Vec<RowError>,
    total: &mut usize,
    line: usize,
    message: impl Into<String>,
) {
    *total += 1;
    if errors.len() < MAX_REPORTED_ERRORS {
        errors.push(RowError {
            line,
            message: message.into(),
        });
    }
}

/// Dates telles que saisies sur le terrain : ISO, mais aussi les formats français
/// qu'Excel écrit par défaut.
pub fn parse_date(value: &str) -> Option<chrono::NaiveDate> {
    const FORMATS: [&str; 5] = ["%Y-%m-%d", "%d/%m/%Y", "%d-%m-%Y", "%d.%m.%Y", "%Y/%m/%d"];
    FORMATS
        .iter()
        .find_map(|format| chrono::NaiveDate::parse_from_str(value, format).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_accented_and_spaced_headers() {
        assert_eq!(normalize_header("Code_Commune_attaché"), "code_commune_attache");
        assert_eq!(normalize_header("  NOM COMPLET "), "nom_complet");
        assert_eq!(normalize_header("N° Matricule"), "n_matricule");
        assert_eq!(normalize_header("Département"), "departement");
    }

    #[test]
    fn detects_french_excel_delimiter() {
        assert_eq!(detect_delimiter("a;b;c\n1;2;3"), b';');
        assert_eq!(detect_delimiter("a,b,c\n1,2,3"), b',');
        assert_eq!(detect_delimiter("a\tb\tc"), b'\t');
    }

    #[test]
    fn strips_utf8_bom() {
        let body = [b"\xEF\xBB\xBF".as_slice(), b"matricule,nom".as_slice()].concat();
        assert_eq!(decode(&body), "matricule,nom");
    }

    #[test]
    fn falls_back_to_latin1_when_not_utf8() {
        // « é » en CP1252 (0xE9) n'est pas de l'UTF-8 valide.
        let body = b"Nom\xE9".to_vec();
        assert_eq!(decode(&body), "Nomé");
    }

    #[test]
    fn parses_french_and_iso_dates() {
        use chrono::NaiveDate;
        let expected = NaiveDate::from_ymd_opt(2024, 3, 12).unwrap();
        assert_eq!(parse_date("2024-03-12"), Some(expected));
        assert_eq!(parse_date("12/03/2024"), Some(expected));
        assert_eq!(parse_date("12-03-2024"), Some(expected));
        assert_eq!(parse_date("pas une date"), None);
    }

    const SPECS: &[ColumnSpec] = &[
        ColumnSpec {
            canonical: "matricule",
            aliases: &["n_matricule", "numero_matricule"],
            loose_aliases: &[],
            required: true,
        },
        ColumnSpec {
            canonical: "full_name",
            aliases: &["nom_complet", "noms_et_prenoms"],
            loose_aliases: &["nom"],
            required: true,
        },
        ColumnSpec {
            canonical: "code_commune",
            aliases: &["code_commune_attache", "commune_code"],
            loose_aliases: &["commune"],
            required: false,
        },
    ];

    #[test]
    fn resolves_client_headers() {
        let headers = csv::StringRecord::from(vec!["Matricule", "Nom_Complet", "Code_Commune_attache"]);
        let columns = resolve_columns(&headers, SPECS).expect("columns");
        assert_eq!(columns.index_of("matricule"), Some(0));
        assert_eq!(columns.index_of("full_name"), Some(1));
        assert_eq!(columns.index_of("code_commune"), Some(2));
    }

    #[test]
    fn strict_code_column_wins_over_loose_name_column() {
        // Cas du fichier exporté puis réimporté : « Commune » (libellé) ET « Code Commune ».
        let headers =
            csv::StringRecord::from(vec!["Matricule", "Nom Complet", "Commune", "Code Commune"]);
        let columns = resolve_columns(&headers, SPECS).expect("columns");
        assert_eq!(
            columns.index_of("code_commune"),
            Some(3),
            "la colonne de code doit primer sur la colonne de libelle"
        );
    }

    #[test]
    fn missing_required_header_is_reported_with_seen_columns() {
        let headers = csv::StringRecord::from(vec!["Nom_Complet", "Code_Commune_attache"]);
        let error = resolve_columns(&headers, SPECS).expect_err("should fail");
        let message = format!("{error:?}");
        assert!(message.contains("matricule"), "message: {message}");
    }

    #[test]
    fn optional_columns_may_be_absent() {
        let headers = csv::StringRecord::from(vec!["matricule", "nom_complet"]);
        let columns = resolve_columns(&headers, SPECS).expect("columns");
        assert!(!columns.has("code_commune"));
    }

    #[test]
    fn blank_rows_are_detected() {
        assert!(is_blank(&csv::StringRecord::from(vec!["", " ", ""])));
        assert!(!is_blank(&csv::StringRecord::from(vec!["", "x"])));
    }

    #[test]
    fn short_rows_do_not_panic() {
        let content = "matricule,nom_complet,code_commune\nAPM-1,Jean\n";
        let mut rdr = reader(content, b',');
        let headers = rdr.headers().expect("headers").clone();
        let columns = resolve_columns(&headers, SPECS).expect("columns");
        let record = rdr.records().next().expect("row").expect("ok");
        assert_eq!(columns.get(&record, "full_name"), Some("Jean"));
        assert_eq!(columns.get(&record, "code_commune"), None);
    }

    #[test]
    fn error_list_is_capped_but_total_keeps_counting() {
        let mut errors = Vec::new();
        let mut total = 0;
        for line in 0..(MAX_REPORTED_ERRORS + 50) {
            push_error(&mut errors, &mut total, line, "boom");
        }
        assert_eq!(errors.len(), MAX_REPORTED_ERRORS);
        assert_eq!(total, MAX_REPORTED_ERRORS + 50);
    }
}
