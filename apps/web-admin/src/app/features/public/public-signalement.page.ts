import { Component, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';

import { ApiService } from '../../core/services/api.service';

@Component({
  selector: 'app-public-signalement-page',
  imports: [ReactiveFormsModule],
  template: `
    <section class="grid gap-5 lg:grid-cols-[0.8fr_1.2fr]">
      <div>
        <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Signalement citoyen</p>
        <h1 class="mt-1 text-3xl font-black">Deposer un signalement</h1>
        <p class="mt-3 text-[var(--text-muted)]">
          Le signalement peut rester anonyme. Un numero de suivi est genere apres validation.
        </p>
      </div>

      <form class="panel grid gap-4 p-5 shadow-[var(--shadow-soft)]" [formGroup]="form" (ngSubmit)="submit()">
        <div class="field">
          <label>Commune ID</label>
          <input formControlName="commune_id" />
        </div>
        <div class="grid gap-4 md:grid-cols-2">
          <div class="field">
            <label>Type incident</label>
            <input formControlName="type_incident" />
          </div>
          <div class="field">
            <label>Lieu</label>
            <input formControlName="location_description" />
          </div>
        </div>
        <div class="field">
          <label>Description</label>
          <textarea formControlName="description"></textarea>
        </div>
        <label class="flex items-center gap-2 text-sm font-semibold">
          <input type="checkbox" formControlName="contact_anonyme" />
          Rester anonyme
        </label>
        <div class="field">
          <label>Contact optionnel</label>
          <input formControlName="contact_info" />
        </div>
        <button type="submit" class="btn-primary" [disabled]="form.invalid || loading()">
          {{ loading() ? 'Envoi...' : 'Envoyer' }}
        </button>

        @if (result()) {
          <p class="rounded-md border border-green-200 bg-green-50 p-3 text-sm font-semibold text-green-800">
            Numero de suivi: {{ result() }}
          </p>
        }
        @if (error()) {
          <p class="rounded-md border border-red-200 bg-red-50 p-3 text-sm font-semibold text-red-800">
            {{ error() }}
          </p>
        }
      </form>
    </section>
  `,
})
export class PublicSignalementPage {
  private readonly fb = inject(FormBuilder);
  private readonly api = inject(ApiService);

  protected readonly loading = signal(false);
  protected readonly result = signal<string | null>(null);
  protected readonly error = signal<string | null>(null);
  protected readonly form = this.fb.nonNullable.group({
    commune_id: ['', Validators.required],
    type_incident: ['', Validators.required],
    location_description: ['', Validators.required],
    description: ['', Validators.required],
    contact_anonyme: true,
    contact_info: '',
  });

  protected submit(): void {
    if (this.form.invalid) {
      return;
    }
    this.loading.set(true);
    this.error.set(null);
    this.api
      .post<{ signalement_number: string }>('/api/v1/public/signalements', this.form.getRawValue())
      .subscribe({
        next: (result) => {
          this.result.set(result.signalement_number);
          this.loading.set(false);
        },
        error: () => {
          this.error.set('Signalement refuse. Verifie les informations saisies.');
          this.loading.set(false);
        },
      });
  }
}
