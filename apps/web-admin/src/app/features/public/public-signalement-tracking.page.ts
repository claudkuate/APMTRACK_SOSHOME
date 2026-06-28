import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { I18nService } from '../../core/i18n/i18n.service';
import { TranslatePipe } from '../../core/i18n/translate.pipe';
import { ApiService } from '../../core/services/api.service';
import { HelpTipComponent } from '../../shared/ui/help-tip.component';
import { PublicEntry, formatPublicEntries, publicStatusClasses } from './public-display';

@Component({
  selector: 'app-public-signalement-tracking-page',
  imports: [FormsModule, TranslatePipe, HelpTipComponent],
  template: `
    <section class="grid gap-5 lg:grid-cols-[0.8fr_1.2fr]">
      <div>
        <p class="text-xs font-bold uppercase text-[var(--text-muted)]">{{ 'public.track.eyebrow' | t }}</p>
        <h1 class="mt-1 flex items-center gap-2 text-3xl font-black">
          {{ 'public.track.title' | t }}
          <app-help-tip [text]="'public.track.help' | t" />
        </h1>
        <p class="mt-3 text-[var(--text-muted)]">{{ 'public.track.subtitle' | t }}</p>
      </div>

      <div class="panel p-5 shadow-[var(--shadow-soft)]">
        <div class="grid gap-3 sm:grid-cols-[1fr_auto]">
          <div class="field">
            <label>{{ 'public.track.label' | t }}</label>
            <input [(ngModel)]="number" placeholder="SIG-YDE1-2026-000001" />
          </div>
          <button type="button" class="btn-primary self-end" (click)="track()">
            {{ 'public.track.submit' | t }}
          </button>
        </div>

        @if (result()) {
          <dl class="mt-5 grid gap-3 sm:grid-cols-2">
            @for (item of entries(); track item.key) {
              <div [class]="cardClass(item)">
                <dt class="text-xs font-bold uppercase text-[var(--text-muted)]">{{ item.label }}</dt>
                <dd class="mt-1 font-black">{{ item.value }}</dd>
              </div>
            }
          </dl>
        }
        @if (error()) {
          <p class="mt-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm font-semibold text-red-800">
            {{ error() }}
          </p>
        }

        @if (result() || error()) {
          <div class="mt-4 flex justify-end">
            <button type="button" class="btn-ghost" (click)="reset()">{{ 'common.close' | t }}</button>
          </div>
        }
      </div>
    </section>
  `,
})
export class PublicSignalementTrackingPage {
  private readonly api = inject(ApiService);
  protected readonly i18n = inject(I18nService);

  protected readonly result = signal<Record<string, unknown> | null>(null);
  protected readonly error = signal<string | null>(null);
  protected number = '';

  protected track(): void {
    const value = this.number.trim();
    if (!value) {
      this.error.set(this.i18n.t('public.track.required'));
      return;
    }
    this.api.get<Record<string, unknown>>(`/api/v1/public/signalements/${encodeURIComponent(value)}`).subscribe({
      next: (result) => {
        this.result.set(result);
        this.error.set(null);
      },
      error: () => {
        this.result.set(null);
        this.error.set(this.i18n.t('public.track.notFound'));
      },
    });
  }

  /** Réinitialise la vue (bouton Fermer — remarque 7). */
  protected reset(): void {
    this.result.set(null);
    this.error.set(null);
    this.number = '';
  }

  protected entries() {
    return formatPublicEntries(this.result(), this.i18n);
  }

  /** Fond coloré pour le champ « Statut », neutre pour les autres. */
  protected cardClass(item: PublicEntry): string {
    const base = 'rounded-md p-3';
    return item.key === 'status'
      ? `${base} ${publicStatusClasses(item.value)}`
      : `${base} bg-[var(--surface-muted)]`;
  }
}
