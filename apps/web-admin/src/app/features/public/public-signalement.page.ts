import { Component, OnDestroy, OnInit, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { Subscription } from 'rxjs';

import { ApiService } from '../../core/services/api.service';
import {
  PublicCommuneOption,
  PublicIncidentTypeOption,
  PublicSignalementOptions,
  PublicZoneOption,
} from '../../shared/api-types';

@Component({
  selector: 'app-public-signalement-page',
  imports: [ReactiveFormsModule],
  template: `
    <section class="grid gap-5 lg:grid-cols-[0.8fr_1.2fr]">
      <div>
        <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Signalement citoyen</p>
        <h1 class="mt-1 text-3xl font-black">Deposer un signalement</h1>
        <p class="mt-3 text-[var(--text-muted)]">
          Un numero de suivi est genere apres validation.
        </p>
      </div>

      <form
        class="panel grid gap-4 p-5 shadow-[var(--shadow-soft)]"
        [formGroup]="form"
        (ngSubmit)="submit()"
      >
        <div class="field">
          <label for="commune_id">Commune</label>
          <select id="commune_id" formControlName="commune_id" (change)="onCommuneChange()">
            <option value="">Choisir...</option>
            @for (option of communes(); track option.id) {
              <option [value]="option.id">
                {{ option.nom }} - {{ option.departement }} - {{ option.region }}
              </option>
            }
          </select>
        </div>

        <div class="grid gap-4 md:grid-cols-2">
          <div class="field">
            <label for="type_incident">Type d'incident</label>
            <select id="type_incident" formControlName="type_incident">
              <option value="">Choisir...</option>
              @for (option of incidentTypes(); track option.id) {
                <option [value]="option.id">{{ option.category_nom }} - {{ option.nom }}</option>
              }
            </select>
          </div>
          <div class="field">
            <label for="zone_id">Quartier / zone</label>
            <select id="zone_id" formControlName="zone_id">
              <option value="">Choisir...</option>
              @for (option of zones(); track option.id) {
                <option [value]="option.id">{{ option.nom }}</option>
              }
            </select>
          </div>
        </div>

        <div class="field">
          <label for="lieu_dit">Lieu-dit</label>
          <input id="lieu_dit" formControlName="lieu_dit" />
        </div>

        <div class="field">
          <label for="description">Description</label>
          <textarea id="description" formControlName="description"></textarea>
        </div>

        <label class="flex items-center gap-2 text-sm font-semibold">
          <input type="checkbox" formControlName="contact_anonyme" />
          Rester anonyme
        </label>

        @if (!form.controls.contact_anonyme.value) {
          <div class="grid gap-4 md:grid-cols-2">
            <div class="field">
              <label for="contact_name">Nom du contact</label>
              <input id="contact_name" formControlName="contact_name" />
            </div>
            <div class="field">
              <label for="contact_phone">Telephone du contact</label>
              <input id="contact_phone" formControlName="contact_phone" />
            </div>
          </div>
        }

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
export class PublicSignalementPage implements OnInit, OnDestroy {
  private readonly fb = inject(FormBuilder);
  private readonly api = inject(ApiService);
  private readonly subscriptions = new Subscription();

  protected readonly loading = signal(false);
  protected readonly result = signal<string | null>(null);
  protected readonly error = signal<string | null>(null);
  protected readonly communes = signal<PublicCommuneOption[]>([]);
  protected readonly incidentTypes = signal<PublicIncidentTypeOption[]>([]);
  protected readonly zones = signal<PublicZoneOption[]>([]);

  protected readonly form = this.fb.nonNullable.group({
    commune_id: ['', Validators.required],
    type_incident: ['', Validators.required],
    zone_id: ['', Validators.required],
    lieu_dit: '',
    location_description: '',
    description: ['', Validators.required],
    contact_anonyme: true,
    contact_name: '',
    contact_phone: '',
  });

  ngOnInit(): void {
    this.api.get<PublicCommuneOption[]>('/api/v1/public/communes', { limit: 400 }).subscribe({
      next: (options) => this.communes.set(options),
      error: () => this.error.set('Impossible de charger les communes disponibles.'),
    });

    this.subscriptions.add(
      this.form.controls.contact_anonyme.valueChanges.subscribe((anonymous) => {
        if (anonymous) {
          this.form.patchValue({ contact_name: '', contact_phone: '' }, { emitEvent: false });
          this.form.controls.contact_name.clearValidators();
          this.form.controls.contact_phone.clearValidators();
        } else {
          this.form.controls.contact_name.setValidators([Validators.required]);
          this.form.controls.contact_phone.setValidators([Validators.required]);
        }
        this.form.controls.contact_name.updateValueAndValidity({ emitEvent: false });
        this.form.controls.contact_phone.updateValueAndValidity({ emitEvent: false });
      }),
    );
  }

  ngOnDestroy(): void {
    this.subscriptions.unsubscribe();
  }

  protected onCommuneChange(): void {
    const communeId = this.form.controls.commune_id.value;
    this.form.patchValue(
      { type_incident: '', zone_id: '', lieu_dit: '', location_description: '' },
      { emitEvent: false },
    );
    this.incidentTypes.set([]);
    this.zones.set([]);
    if (!communeId) {
      return;
    }
    this.api
      .get<PublicSignalementOptions>(`/api/v1/public/communes/${communeId}/signalement-options`)
      .subscribe({
        next: (options) => {
          this.incidentTypes.set(options.incident_types);
          this.zones.set(options.zones);
        },
        error: () => this.error.set('Options indisponibles pour cette commune.'),
      });
  }

  protected submit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }
    this.loading.set(true);
    this.error.set(null);

    const raw = this.form.getRawValue();
    const zoneName = this.zones().find((zone) => zone.id === raw.zone_id)?.nom ?? '';
    const location = [zoneName, raw.lieu_dit.trim()].filter(Boolean).join(' - ');
    const payload = {
      ...raw,
      location_description: location,
      contact_name: raw.contact_anonyme ? null : raw.contact_name,
      contact_phone: raw.contact_anonyme ? null : raw.contact_phone,
    };

    this.api.post<{ signalement_number: string }>('/api/v1/public/signalements', payload).subscribe({
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
