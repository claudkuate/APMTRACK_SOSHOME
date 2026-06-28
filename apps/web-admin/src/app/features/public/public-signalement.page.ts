import { Component, OnDestroy, OnInit, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { Subscription } from 'rxjs';

import { I18nService } from '../../core/i18n/i18n.service';
import { TranslatePipe } from '../../core/i18n/translate.pipe';
import { ApiService } from '../../core/services/api.service';
import { HelpTipComponent } from '../../shared/ui/help-tip.component';
import {
  PublicCommuneOption,
  PublicDepartementOption,
  PublicRegionOption,
  PublicSignalementOptions,
  PublicZoneOption,
} from '../../shared/api-types';

/** Type d'action contestée — liste fixe alignée sur le backend (libellés canoniques). */
interface ComplaintTypeOption {
  value: string;
  labelKey: string;
}

@Component({
  selector: 'app-public-signalement-page',
  imports: [ReactiveFormsModule, TranslatePipe, HelpTipComponent],
  template: `
    <section class="grid gap-5 lg:grid-cols-[0.8fr_1.2fr]">
      <div>
        <p class="text-xs font-bold uppercase text-[var(--text-muted)]">{{ 'public.report.eyebrow' | t }}</p>
        <h1 class="mt-1 flex items-center gap-2 text-3xl font-black">
          {{ 'public.report.title' | t }}
          <app-help-tip [text]="'public.report.help' | t" />
        </h1>
        <p class="mt-3 text-[var(--text-muted)]">{{ 'public.report.subtitle' | t }}</p>
      </div>

      <form class="panel grid gap-4 p-5 shadow-[var(--shadow-soft)]" [formGroup]="form" (ngSubmit)="submit()">
        <!-- Cascade géographique : Région → Département → Commune (remarque 13) -->
        <div class="grid gap-4 md:grid-cols-2">
          <div class="field">
            <label for="region_id">{{ 'public.report.region' | t }}</label>
            <select id="region_id" formControlName="region_id" (change)="onRegionChange()">
              <option value="">{{ 'common.choose' | t }}</option>
              @for (option of regions(); track option.id) {
                <option [value]="option.id">{{ option.nom }}</option>
              }
            </select>
          </div>
          <div class="field">
            <label for="departement_id">{{ 'public.report.departement' | t }}</label>
            <select
              id="departement_id"
              formControlName="departement_id"
              (change)="onDepartementChange()"
              [disabled]="!departements().length"
            >
              <option value="">{{ 'common.choose' | t }}</option>
              @for (option of departements(); track option.id) {
                <option [value]="option.id">{{ option.nom }}</option>
              }
            </select>
          </div>
        </div>

        <div class="field">
          <label for="commune_id">{{ 'public.report.commune' | t }}</label>
          <select id="commune_id" formControlName="commune_id" (change)="onCommuneChange()" [disabled]="!communes().length">
            <option value="">{{ 'common.choose' | t }}</option>
            @for (option of communes(); track option.id) {
              <option [value]="option.id">{{ option.nom }}</option>
            }
          </select>
        </div>

        <!-- Type d'action contestée — liste fixe (remarque : plainte / contre-PV) -->
        <div class="field">
          <label for="type_incident">{{ 'public.report.type' | t }}</label>
          <select id="type_incident" formControlName="type_incident">
            <option value="">{{ 'common.choose' | t }}</option>
            @for (option of complaintTypes; track option.value) {
              <option [value]="option.value">{{ option.labelKey | t }}</option>
            }
          </select>
        </div>

        <!-- Agent visé par la plainte -->
        <div class="grid gap-4 md:grid-cols-2">
          <div class="field">
            <label for="reported_agent_matricule">{{ 'public.report.reportedAgentMat' | t }}</label>
            <input id="reported_agent_matricule" formControlName="reported_agent_matricule" placeholder="APM-YDE7-001" />
          </div>
          <div class="field">
            <label for="reported_agent_nom">{{ 'public.report.reportedAgentName' | t }}</label>
            <input id="reported_agent_nom" formControlName="reported_agent_nom" />
          </div>
        </div>

        <div class="grid gap-4 md:grid-cols-2">
          <div class="field">
            <label for="incident_datetime">{{ 'public.report.incidentDate' | t }}</label>
            <input id="incident_datetime" type="datetime-local" formControlName="incident_datetime" />
          </div>
          <div class="field">
            <label for="pv_number_ref">{{ 'public.report.pvRef' | t }}</label>
            <input id="pv_number_ref" formControlName="pv_number_ref" placeholder="PV-…" />
          </div>
        </div>

        <div class="grid gap-4 md:grid-cols-2">
          <div class="field">
            <label for="zone_id">{{ 'public.report.zone' | t }}</label>
            <select id="zone_id" formControlName="zone_id" [disabled]="!zones().length">
              <option value="">{{ 'common.choose' | t }}</option>
              @for (option of zones(); track option.id) {
                <option [value]="option.id">{{ option.nom }}</option>
              }
            </select>
          </div>
          <div class="field">
            <label for="lieu_dit">{{ 'public.report.lieuDit' | t }}</label>
            <input id="lieu_dit" formControlName="lieu_dit" />
          </div>
        </div>

        <div class="field">
          <label for="description">{{ 'public.report.description' | t }}</label>
          <textarea id="description" formControlName="description"></textarea>
        </div>

        <label class="flex items-center gap-2 text-sm font-semibold">
          <input type="checkbox" formControlName="contact_anonyme" />
          {{ 'public.report.anonymous' | t }}
        </label>

        @if (!form.controls.contact_anonyme.value) {
          <div class="grid gap-4 md:grid-cols-2">
            <div class="field">
              <label for="contact_name">{{ 'public.report.contactName' | t }}</label>
              <input id="contact_name" formControlName="contact_name" />
            </div>
            <div class="field">
              <label for="contact_phone">{{ 'public.report.contactPhone' | t }}</label>
              <input id="contact_phone" formControlName="contact_phone" />
            </div>
          </div>
        }

        @if (result()) {
          <div class="grid gap-2 rounded-md border border-green-200 bg-green-50 p-3 text-sm text-green-800">
            <p class="font-semibold">{{ 'public.report.tracking' | t: { value: result()! } }}</p>
            <p>{{ 'public.report.successNote' | t }}</p>
          </div>
          <button type="button" class="btn-ghost" (click)="reset()">{{ 'public.report.newReport' | t }}</button>
        } @else {
          <button type="submit" class="btn-primary" [disabled]="form.invalid || loading()">
            {{ (loading() ? 'public.report.submitting' : 'public.report.submit') | t }}
          </button>
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
  protected readonly i18n = inject(I18nService);
  private readonly subscriptions = new Subscription();

  protected readonly loading = signal(false);
  protected readonly result = signal<string | null>(null);
  protected readonly error = signal<string | null>(null);
  protected readonly regions = signal<PublicRegionOption[]>([]);
  protected readonly departements = signal<PublicDepartementOption[]>([]);
  protected readonly communes = signal<PublicCommuneOption[]>([]);
  protected readonly zones = signal<PublicZoneOption[]>([]);

  /** Types d'action contestée (alignés sur `COMPLAINT_TYPES` côté API). */
  protected readonly complaintTypes: ComplaintTypeOption[] = [
    { value: 'Amende', labelKey: 'public.report.type.amende' },
    { value: 'Verbalisation', labelKey: 'public.report.type.verbalisation' },
    { value: 'Mise sous scellé', labelKey: 'public.report.type.scelle' },
    { value: 'Mise en fourrière', labelKey: 'public.report.type.fourriere' },
    { value: 'Autre', labelKey: 'public.report.type.autre' },
  ];

  protected readonly form = this.fb.nonNullable.group({
    region_id: ['', Validators.required],
    departement_id: ['', Validators.required],
    commune_id: ['', Validators.required],
    type_incident: ['', Validators.required],
    reported_agent_matricule: '',
    reported_agent_nom: '',
    incident_datetime: '',
    pv_number_ref: '',
    zone_id: ['', Validators.required],
    lieu_dit: '',
    description: ['', Validators.required],
    contact_anonyme: true,
    contact_name: '',
    contact_phone: '',
  });

  ngOnInit(): void {
    this.api.get<PublicRegionOption[]>('/api/v1/public/geography/regions').subscribe({
      next: (options) => this.regions.set(options),
      error: () => this.error.set(this.i18n.t('public.report.geoError')),
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

  protected onRegionChange(): void {
    const regionId = this.form.controls.region_id.value;
    this.form.patchValue(
      { departement_id: '', commune_id: '', zone_id: '', lieu_dit: '' },
      { emitEvent: false },
    );
    this.departements.set([]);
    this.communes.set([]);
    this.zones.set([]);
    if (!regionId) {
      return;
    }
    this.api
      .get<PublicDepartementOption[]>(`/api/v1/public/geography/regions/${regionId}/departements`)
      .subscribe({
        next: (options) => this.departements.set(options),
        error: () => this.error.set(this.i18n.t('public.report.geoError')),
      });
  }

  protected onDepartementChange(): void {
    const departementId = this.form.controls.departement_id.value;
    this.form.patchValue({ commune_id: '', zone_id: '', lieu_dit: '' }, { emitEvent: false });
    this.communes.set([]);
    this.zones.set([]);
    if (!departementId) {
      return;
    }
    this.api
      .get<PublicCommuneOption[]>(`/api/v1/public/geography/departements/${departementId}/communes`)
      .subscribe({
        next: (options) => this.communes.set(options),
        error: () => this.error.set(this.i18n.t('public.report.geoError')),
      });
  }

  protected onCommuneChange(): void {
    const communeId = this.form.controls.commune_id.value;
    this.form.patchValue({ zone_id: '', lieu_dit: '' }, { emitEvent: false });
    this.zones.set([]);
    if (!communeId) {
      return;
    }
    this.api
      .get<PublicSignalementOptions>(`/api/v1/public/communes/${communeId}/signalement-options`)
      .subscribe({
        next: (options) => this.zones.set(options.zones),
        error: () => this.error.set(this.i18n.t('public.report.optionsError')),
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
      commune_id: raw.commune_id,
      zone_id: raw.zone_id,
      type_incident: raw.type_incident,
      lieu_dit: raw.lieu_dit,
      location_description: location,
      description: raw.description,
      reported_agent_matricule: raw.reported_agent_matricule.trim() || null,
      reported_agent_nom: raw.reported_agent_nom.trim() || null,
      incident_datetime: raw.incident_datetime ? new Date(raw.incident_datetime).toISOString() : null,
      pv_number_ref: raw.pv_number_ref.trim() || null,
      contact_anonyme: raw.contact_anonyme,
      contact_name: raw.contact_anonyme ? null : raw.contact_name,
      contact_phone: raw.contact_anonyme ? null : raw.contact_phone,
    };

    this.api.post<{ signalement_number: string }>('/api/v1/public/signalements', payload).subscribe({
      next: (result) => {
        this.result.set(result.signalement_number);
        this.loading.set(false);
      },
      error: () => {
        this.error.set(this.i18n.t('public.report.error'));
        this.loading.set(false);
      },
    });
  }

  /** Réinitialise le formulaire pour un nouveau signalement (remarque 7). */
  protected reset(): void {
    this.form.reset({ contact_anonyme: true });
    this.departements.set([]);
    this.communes.set([]);
    this.zones.set([]);
    this.result.set(null);
    this.error.set(null);
  }
}
