export interface RuntimeConfig {
  apiUrl?: string;
  environment?: string;
  /** Numéro d'infoline affiché sur le portail public (format libre). */
  infolinePhone?: string;
  /** Numéro WhatsApp pour le lien click-to-chat (chiffres, indicatif inclus). */
  whatsappNumber?: string;
}

/** Coordonnées de contact prêtes à l'emploi pour le portail public. */
export interface ContactConfig {
  /** Affichage lisible de l'infoline. */
  infolinePhone: string;
  /** Valeur du lien `tel:` (indicatif + chiffres, sans espaces). */
  infolineTel: string;
  /** Numéro WhatsApp normalisé (chiffres uniquement) pour `https://wa.me/`. */
  whatsappNumber: string;
}

declare global {
  interface Window {
    __APMTRACK_CONFIG__?: RuntimeConfig;
  }
}

export function apiBaseUrl(): string {
  return normalizeUrl(window.__APMTRACK_CONFIG__?.apiUrl ?? 'http://localhost:8080');
}

export function runtimeEnvironment(): string {
  return window.__APMTRACK_CONFIG__?.environment ?? 'development';
}

export function contactConfig(): ContactConfig {
  const config = window.__APMTRACK_CONFIG__;
  const infolinePhone = config?.infolinePhone ?? '+237 650 19 47 74';
  const whatsappSource = config?.whatsappNumber ?? infolinePhone;
  return {
    infolinePhone,
    infolineTel: telDigits(infolinePhone),
    whatsappNumber: digitsOnly(whatsappSource),
  };
}

function normalizeUrl(url: string): string {
  return url.endsWith('/') ? url.slice(0, -1) : url;
}

/** Conserve un éventuel `+` initial puis les chiffres (valeur d'un lien `tel:`). */
function telDigits(value: string): string {
  const digits = digitsOnly(value);
  return value.trim().startsWith('+') ? `+${digits}` : digits;
}

function digitsOnly(value: string): string {
  return value.replace(/\D/g, '');
}

