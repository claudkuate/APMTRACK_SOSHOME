import { Injectable, signal } from '@angular/core';

import { ADMIN_EN } from './admin-en';
import { EN } from './locales/en';
import { FR } from './locales/fr';

export type Lang = 'fr' | 'en';

const STORAGE_KEY = 'apmtrack.lang';

const DICTIONARIES: Record<Lang, Record<string, string>> = { fr: FR, en: EN };

/**
 * Service i18n maison (sans dépendance externe) : une langue active partagée
 * entre l'espace public et le back-office, persistée dans `localStorage`.
 * Les libellés sont des clés pointées résolues via {@link t}; le repli se fait
 * sur le français puis sur la clé brute si une traduction manque.
 */
@Injectable({ providedIn: 'root' })
export class I18nService {
  readonly lang = signal<Lang>(readStoredLang());

  setLang(value: string): void {
    const lang: Lang = value === 'en' ? 'en' : 'fr';
    localStorage.setItem(STORAGE_KEY, lang);
    this.lang.set(lang);
  }

  toggle(): void {
    this.setLang(this.lang() === 'en' ? 'fr' : 'en');
  }

  /** Traduit une clé, avec interpolation optionnelle de `{param}`. */
  t(key: string, params?: Record<string, string | number>): string {
    const lang = this.lang();
    const template = DICTIONARIES[lang][key] ?? DICTIONARIES.fr[key] ?? key;
    if (!params) {
      return template;
    }
    return template.replace(/\{(\w+)\}/g, (match, name: string) =>
      name in params ? String(params[name]) : match,
    );
  }

  private locale(): string {
    return this.lang() === 'en' ? 'en-GB' : 'fr-FR';
  }

  /** Formate une date ISO selon la langue active (repli sur la valeur brute). */
  formatDate(value: string): string {
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? value : date.toLocaleString(this.locale());
  }

  /** Formate un montant entier FCFA selon la langue active. */
  formatMoneyFcfa(value: unknown): string {
    return `${Number(value).toLocaleString(this.locale())} FCFA`;
  }

  /** Rend un booléen sous forme Oui/Non (ou Yes/No). */
  yesNo(value: boolean): string {
    return this.t(value ? 'common.yes' : 'common.no');
  }

  /**
   * Traduit un libellé FRANÇAIS du back-office vers l'anglais via {@link ADMIN_EN}.
   * En français (ou si aucune traduction n'existe), renvoie le texte source.
   * Utilisé pour bilinguiser l'admin piloté par configuration sans le réécrire.
   */
  auto(value: string | null | undefined): string {
    if (!value) {
      return value ?? '';
    }
    return this.lang() === 'en' ? (ADMIN_EN[value] ?? value) : value;
  }
}

function readStoredLang(): Lang {
  return localStorage.getItem(STORAGE_KEY) === 'en' ? 'en' : 'fr';
}
