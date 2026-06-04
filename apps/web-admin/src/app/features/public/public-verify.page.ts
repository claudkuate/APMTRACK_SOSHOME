import { Component, OnInit, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ActivatedRoute } from '@angular/router';

import { ApiService } from '../../core/services/api.service';

@Component({
  selector: 'app-public-verify-page',
  imports: [FormsModule],
  template: `
    <section class="grid min-w-0 gap-5 lg:grid-cols-[0.8fr_1.2fr]">
      <div class="min-w-0">
        <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Verification publique</p>
        <h1 class="mt-1 break-words text-2xl font-black sm:text-3xl">
          {{ mode() === 'agent' ? 'Verifier un agent' : 'Verifier un PV' }}
        </h1>
        <p class="mt-3 text-[var(--text-muted)]">
          Les donnees affichees sont limitees aux informations utiles a la verification publique.
        </p>
      </div>

      <div class="panel min-w-0 p-5 shadow-[var(--shadow-soft)]">
        <div class="grid gap-3 sm:grid-cols-[1fr_auto]">
          <div class="field min-w-0">
            <label>{{ mode() === 'agent' ? 'Matricule agent' : 'Numero PV' }}</label>
            <input [(ngModel)]="value" [placeholder]="mode() === 'agent' ? 'APM-YDE1-001' : 'PV-YDE1-2026-000001'" />
          </div>
          <button type="button" class="btn-primary w-full self-end sm:w-auto" (click)="verify()">Verifier</button>
        </div>

        @if (error()) {
          <p class="mt-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm font-semibold text-red-800">
            {{ error() }}
          </p>
        }

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
      </div>
    </section>
  `,
})
export class PublicVerifyPage implements OnInit {
  private readonly route = inject(ActivatedRoute);
  private readonly api = inject(ApiService);

  protected readonly mode = signal<'agent' | 'pv'>('agent');
  protected readonly result = signal<Record<string, unknown> | null>(null);
  protected readonly error = signal<string | null>(null);
  protected value = '';

  ngOnInit(): void {
    this.route.data.subscribe((data) => {
      this.mode.set(data['mode'] === 'pv' ? 'pv' : 'agent');
      this.result.set(null);
      this.error.set(null);
      this.value = '';
    });
  }

  protected verify(): void {
    const value = this.value.trim();
    if (!value) {
      this.error.set('Valeur requise.');
      return;
    }
    const path =
      this.mode() === 'agent'
        ? `/api/v1/public/agents/verify/${encodeURIComponent(value)}`
        : `/api/v1/public/pvs/${encodeURIComponent(value)}`;
    this.api.get<Record<string, unknown>>(path).subscribe({
      next: (result) => {
        this.result.set(result);
        this.error.set(null);
      },
      error: () => {
        this.result.set(null);
        this.error.set('Aucun resultat verifiable pour cette reference.');
      },
    });
  }

  protected entries() {
    return Object.entries(this.result() ?? {}).map(([key, value]) => ({
      key,
      value: this.display(value),
    }));
  }

  private display(value: unknown): string {
    if (value === null || value === undefined) {
      return '-';
    }
    if (typeof value === 'boolean') {
      return value ? 'Oui' : 'Non';
    }
    return String(value);
  }
}
