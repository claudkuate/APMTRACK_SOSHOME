/**
 * Export CSV côté client (vue courante, journal de caisse…).
 *
 * Miroir de `csv_safe_field` de l'API (`apps/api/src/helpers.rs`) : les valeurs
 * commençant par `= + - @` (ou un caractère de contrôle) sont préfixées d'une
 * apostrophe pour neutraliser l'injection de formule à l'ouverture dans un
 * tableur. Toutes les cellules sont ensuite mises entre guillemets.
 */
export function csvCell(value: string): string {
  const guarded = /^[=+\-@\t\r\n]/.test(value) ? `'${value}` : value;
  return `"${guarded.replace(/"/g, '""')}"`;
}

/** Construit le contenu CSV (la première ligne de `rows` est l'en-tête). */
export function toCsv(rows: string[][]): string {
  return rows.map((row) => row.map(csvCell).join(',')).join('\n');
}

/** Déclenche le téléchargement d'un CSV construit côté client. */
export function downloadCsv(filename: string, rows: string[][]): void {
  const blob = new Blob([toCsv(rows)], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename.replace(/[^a-z0-9._-]+/gi, '-').toLowerCase();
  link.click();
  URL.revokeObjectURL(url);
}
