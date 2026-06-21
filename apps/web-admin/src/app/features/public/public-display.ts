import { isUuidLike } from '../../shared/resource-config';

/** Libellés lisibles pour les champs exposés dans l'espace public. */
export const PUBLIC_LABELS: Record<string, string> = {
  matricule: 'Matricule',
  full_name: 'Nom',
  status: 'Statut',
  active: 'En activité',
  commune_nom: 'Commune',
  commune_code: 'Code commune',
  pv_number: 'Numéro PV',
  amount_initial_fcfa: 'Montant',
  signalement_number: 'Numéro de suivi',
  type_incident: 'Type d’incident',
  created_at: 'Créé le',
  updated_at: 'Mis à jour le',
};

/**
 * Clés internes/techniques jamais affichées dans l'espace public
 * (drapeaux d'UI, doublons d'affichage).
 */
const HIDDEN_KEYS = new Set(['has_photo', 'amount_initial']);

export interface PublicEntry {
  key: string;
  label: string;
  value: string;
}

/**
 * Transforme une réponse publique brute en lignes affichables :
 * masque les identifiants (clés `id`/`*_id` et valeurs UUID), applique des
 * libellés français et formate booléens, dates et montants.
 */
export function formatPublicEntries(result: Record<string, unknown> | null): PublicEntry[] {
  if (!result) {
    return [];
  }
  return Object.entries(result)
    .filter(([key, value]) => !isHiddenField(key, value))
    .map(([key, value]) => ({
      key,
      label: PUBLIC_LABELS[key] ?? humanizeKey(key),
      value: formatValue(key, value),
    }));
}

function isHiddenField(key: string, value: unknown): boolean {
  if (HIDDEN_KEYS.has(key)) {
    return true;
  }
  if (key === 'id' || key.endsWith('_id')) {
    return true;
  }
  return isUuidLike(value);
}

function formatValue(key: string, value: unknown): string {
  if (value === null || value === undefined || value === '') {
    return '-';
  }
  if (typeof value === 'boolean') {
    return value ? 'Oui' : 'Non';
  }
  if (key.endsWith('_fcfa')) {
    return `${Number(value).toLocaleString('fr-FR')} FCFA`;
  }
  if (typeof value === 'string' && value.includes('T') && value.endsWith('Z')) {
    return new Date(value).toLocaleString('fr-FR');
  }
  return String(value);
}

function humanizeKey(key: string): string {
  return key
    .split('_')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}
