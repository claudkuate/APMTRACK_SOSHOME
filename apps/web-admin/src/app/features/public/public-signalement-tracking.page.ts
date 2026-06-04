import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { ApiService } from '../../core/services/api.service';

@Component({
  selector: 'app-public-signalement-tracking-page',
  imports: [FormsModule],
  template: `
    <section class="grid gap-5 lg:grid-cols-[0.8fr_1.2fr]">
      <div>
        <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Suivi public</p>
        <h1 class="mt-1 text-3xl font-black">Suivre un signalement</h1>
        <p class="mt-3 text-[var(--text-muted)]">
          Le suivi public n'expose pas les notes administratives ni les contacts.
        </p>
      </div>

      <div class="panel p-5 shadow-[var(--shadow-soft)]">
        <div class="grid gap-3 sm:grid-cols-[1fr_auto]">
          <div class="field">
            <label>Numero de suivi</label>
            <input [(ngModel)]="number" placeholder="SIG-YDE1-2026-000001" />
          </div>
          <button type="button" class="btn-primary self-end" (click)="track()">Consulter</button>
        </div>

        @if (result()) {
          <dl class="mt-5 grid gap-3 sm:grid-cols-2">
            @for (item of entries(); track item.key) {
              <div class="rounded-md bg-[var(--surface-muted)] p-3">
                <dt class="text-xs font-bold uppercase text-[var(--text-muted)]">{{ item.key }}</dt>
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
      </div>
    </section>
  `,
})
export class PublicSignalementTrackingPage {
  private readonly api = inject(ApiService);

  protected readonly result = signal<Record<string, unknown> | null>(null);
  protected readonly error = signal<string | null>(null);
  protected number = '';

  protected track(): void {
    const value = this.number.trim();
    if (!value) {
      this.error.set('Numero requis.');
      return;
    }
    this.api.get<Record<string, unknown>>(`/api/v1/public/signalements/${encodeURIComponent(value)}`).subscribe({
      next: (result) => {
        this.result.set(result);
        this.error.set(null);
      },
      error: () => {
        this.result.set(null);
        this.error.set('Signalement introuvable.');
      },
    });
  }

  protected entries() {
    return Object.entries(this.result() ?? {}).map(([key, value]) => ({ key, value: String(value ?? '-') }));
  }
}
