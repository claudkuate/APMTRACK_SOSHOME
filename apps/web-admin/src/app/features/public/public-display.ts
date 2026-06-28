import { I18nService } from '../../core/i18n/i18n.service';
import { isUuidLike } from '../../shared/resource-config';

/** Clés i18n des libellés exposés dans l'espace public. */
export const PUBLIC_LABEL_KEYS: Record<string, string> = {
  matricule: 'field.matricule',
  full_name: 'field.full_name',
  status: 'field.status',
  active: 'field.active',
  commune_nom: 'field.commune_nom',
  pv_number: 'field.pv_number',
  agent_matricule: 'field.agent_matricule',
  amount_initial_fcfa: 'field.amount_initial_fcfa',
  signalement_number: 'field.signalement_number',
  type_incident: 'field.type_incident',
  created_at: 'field.created_at',
  updated_at: 'field.updated_at',
};

/**
 * Classes Tailwind colorant le fond du champ « Statut » pour attirer
 * l'attention (PV : ÉMIS / PAYÉ / EN_RETARD / ANNULÉ — signalements : RECU /
 * EN_COURS / TRAITE / CLASSE / REJETE). Valeur inconnue → neutre.
 */
export function publicStatusClasses(value: unknown): string {
  const neutral = 'bg-slate-200 text-slate-800 ring-1 ring-slate-300';
  const palette: Record<string, string> = {
    green: 'bg-green-100 text-green-900 ring-1 ring-green-300',
    red: 'bg-red-100 text-red-900 ring-1 ring-red-300',
    amber: 'bg-amber-100 text-amber-900 ring-1 ring-amber-300',
    blue: 'bg-blue-100 text-blue-900 ring-1 ring-blue-300',
    slate: neutral,
  };
  const tone: Record<string, keyof typeof palette> = {
    PAYE: 'green',
    TRAITE: 'green',
    EMIS: 'blue',
    RECU: 'blue',
    EN_COURS: 'amber',
    PARTIEL: 'amber',
    CONTESTE: 'amber',
    EN_RETARD: 'red',
    REJETE: 'red',
    ANNULE: 'slate',
    CLASSE: 'slate',
  };
  const key = String(value ?? '').trim().toUpperCase();
  return palette[tone[key] ?? 'slate'];
}

/**
 * Clés internes/techniques jamais affichées dans l'espace public
 * (drapeaux d'UI, doublons d'affichage, champs superflus — remarque 6 :
 * « Code commune »).
 */
const HIDDEN_KEYS = new Set(['has_photo', 'amount_initial', 'commune_code']);

export interface PublicEntry {
  key: string;
  label: string;
  value: string;
}

/**
 * Transforme une réponse publique brute en lignes affichables :
 * masque les identifiants (clés `id`/`*_id` et valeurs UUID), applique des
 * libellés traduits et formate booléens, dates et montants selon la langue.
 */
export function formatPublicEntries(
  result: Record<string, unknown> | null,
  i18n: I18nService,
): PublicEntry[] {
  if (!result) {
    return [];
  }
  return Object.entries(result)
    .filter(([key, value]) => !isHiddenField(key, value))
    .map(([key, value]) => ({
      key,
      label: PUBLIC_LABEL_KEYS[key] ? i18n.t(PUBLIC_LABEL_KEYS[key]) : humanizeKey(key),
      value: formatValue(key, value, i18n),
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

function formatValue(key: string, value: unknown, i18n: I18nService): string {
  if (value === null || value === undefined || value === '') {
    return '-';
  }
  if (typeof value === 'boolean') {
    return i18n.yesNo(value);
  }
  if (key.endsWith('_fcfa')) {
    return i18n.formatMoneyFcfa(value);
  }
  if (typeof value === 'string' && value.includes('T') && value.endsWith('Z')) {
    return i18n.formatDate(value);
  }
  return String(value);
}

function humanizeKey(key: string): string {
  return key
    .split('_')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}
