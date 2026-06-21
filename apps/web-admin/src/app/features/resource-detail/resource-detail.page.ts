import { Component, OnDestroy, OnInit, inject, signal } from '@angular/core';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { Subscription } from 'rxjs';

import { ApiService } from '../../core/services/api.service';
import { GeoGeometry } from '../../core/services/geo.service';
import { LookupService } from '../../core/services/lookup.service';
import {
  RelatedSection,
  ResourceConfig,
  featureForEntityType,
  isUuidLike,
  resourceConfigs,
} from '../../shared/resource-config';
import { MiniMapComponent } from '../../shared/map/mini-map.component';
import { PatrouilleAgentsDialog } from '../patrouilles/patrouille-agents.dialog';

type Row = Record<string, unknown>;

interface RelatedState {
  loading: boolean;
  error: string | null;
  rows: Row[];
}

/** Champs candidats au titre métier d'une ligne (jamais un id). */
const TITLE_KEYS = [
  'pv_number',
  'signalement_number',
  'matricule',
  'nom',
  'full_name',
  'email',
  'receipt_number',
];

const STATUS_LABELS: Record<string, string> = {
  ACTIF: 'Actif',
  SUSPENDU: 'Suspendu',
  RETRAITE: 'Retraite',
  BROUILLON: 'Brouillon',
  EMIS: 'Émis',
  EN_ATTENTE_PAIEMENT: 'En attente paiement',
  PAYE: 'Payé',
  EN_RETARD: 'En retard',
  NON_PAYANT: 'Non payant',
  ANNULE: 'Annulé',
  CONTESTE: 'Contesté',
  RECU: 'Reçu',
  EN_COURS: 'En cours',
  TRAITE: 'Traité',
  CLASSE: 'Classé',
  REJETE: 'Rejeté',
  PLANIFIEE: 'Planifiée',
  CLOTUREE: 'Clôturée',
  ANNULEE: 'Annulée',
};

@Component({
  selector: 'app-resource-detail-page',
  imports: [RouterLink, MiniMapComponent, PatrouilleAgentsDialog],
  template: `
    @if (config(); as cfg) {
      <section class="grid gap-5">
        <nav class="flex items-center gap-2 text-xs font-bold uppercase text-[var(--text-muted)]">
          <a [routerLink]="['/', cfg.key]" class="hover:underline">{{ cfg.title }}</a>
          <span aria-hidden="true">›</span>
          <span class="text-[var(--text-strong)]">{{ title() }}</span>
        </nav>

        <div class="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h2 class="text-2xl font-black">{{ title() }}</h2>
            <p class="mt-1 max-w-3xl text-sm text-[var(--text-muted)]">{{ cfg.description }}</p>
            @if (row(); as item) {
              <div class="mt-3 flex flex-wrap gap-2">
                @for (field of summaryFields(cfg); track field) {
                  <span class="rounded-md bg-[var(--surface-canvas)] px-3 py-1 text-xs">
                    <span class="font-bold text-[var(--text-muted)]">{{ label(cfg, field) }}:</span>
                    <span class="ml-1 font-semibold">{{ display(field, item[field]) }}</span>
                  </span>
                }
              </div>
            }
          </div>
          <a [routerLink]="['/', cfg.key]" class="btn-secondary" aria-label="Retour à la liste">
            Retour liste
          </a>
        </div>

        @if (error()) {
          <div class="panel border-red-200 bg-red-50 p-3 text-sm font-semibold text-red-800">
            {{ error() }}
          </div>
        }

        @if (loading()) {
          <div class="panel p-5 text-[var(--text-muted)]">Chargement du détail...</div>
        } @else if (row(); as item) {
          <nav class="settings-tabs" aria-label="Sections de la fiche">
            @for (tab of tabs(cfg, item); track tab.key) {
              <button
                type="button"
                class="settings-tab"
                [class.is-active]="effectiveTab(cfg, item) === tab.key"
                (click)="activeTab.set(tab.key)"
              >
                {{ tab.label }}
              </button>
            }
          </nav>

          @if (effectiveTab(cfg, item) === 'informations') {
          <section class="panel overflow-hidden">
            <header class="border-b border-[var(--line-subtle)] p-5">
              <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Informations</p>
              <h3 class="mt-1 text-xl font-black">{{ title() }}</h3>
            </header>
            @if (cfg.photoEndpoint && item['id']) {
              <div class="flex flex-wrap items-center gap-4 border-b border-[var(--line-subtle)] p-5">
                <div
                  class="h-24 w-24 flex-none overflow-hidden rounded-lg border border-[var(--line-subtle)] bg-[var(--surface-muted)]"
                >
                  @if (avatarUrl(); as url) {
                    <img [src]="url" alt="Photo de profil" class="h-full w-full object-cover" />
                  } @else {
                    <div class="grid h-full w-full place-items-center text-center text-xs text-[var(--text-muted)]">
                      Aucune photo
                    </div>
                  }
                </div>
                <div class="grid gap-2">
                  <label class="btn-secondary cursor-pointer">
                    {{ uploading() ? 'Envoi...' : 'Changer la photo' }}
                    <input
                      type="file"
                      accept="image/*"
                      class="sr-only"
                      [disabled]="uploading()"
                      (change)="onPhotoSelected($event, cfg, item)"
                    />
                  </label>
                  <span class="text-xs text-[var(--text-muted)]">JPG, PNG ou WebP — 5 Mo max.</span>
                  @if (photoError()) {
                    <span class="text-xs font-semibold text-red-700">{{ photoError() }}</span>
                  }
                </div>
              </div>
            }
            <dl class="grid gap-3 p-5 md:grid-cols-2 xl:grid-cols-3">
              @for (field of detailFields(cfg); track field) {
                <div class="detail-item">
                  <dt>{{ label(cfg, field) }}</dt>
                  <dd>
                    @if (field === 'entity_id') {
                      @if (entityLink(item); as link) {
                        <a
                          [routerLink]="link"
                          class="font-semibold text-[var(--cameroon-red)] hover:underline"
                        >
                          Voir la fiche
                        </a>
                      } @else {
                        <span>{{ display('entity_type', item['entity_type']) }}</span>
                      }
                    } @else {
                      <span [class]="badgeClass(field, item[field])">{{
                        display(field, item[field])
                      }}</span>
                    }
                  </dd>
                </div>
              }
            </dl>
          </section>
          }

          @if (effectiveTab(cfg, item) === 'localisation' && hasLocation(item)) {
            <section class="panel overflow-hidden">
              <header class="border-b border-[var(--line-subtle)] p-5">
                <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Localisation</p>
                <h3 class="mt-1 text-xl font-black">Carte</h3>
              </header>
              <div class="p-5">
                <app-mini-map
                  [latitude]="pointLat(item)"
                  [longitude]="pointLon(item)"
                  [boundary]="boundaryGeometry(item)"
                  [layer]="mapLayer(cfg)"
                  height="320px"
                />
              </div>
            </section>
          }

          @if (effectiveTab(cfg, item) === 'liens') {
          @for (section of relatedSections(cfg); track section.key) {
            @if (childConfig(section); as child) {
              <section class="panel overflow-hidden">
                <header
                  class="flex items-center justify-between border-b border-[var(--line-subtle)] p-5"
                >
                  <div>
                    <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Liens</p>
                    <h3 class="mt-1 text-xl font-black">{{ section.title }}</h3>
                  </div>
                  <a [routerLink]="['/', child.key]" class="btn-ghost text-xs">Voir tout</a>
                </header>
                @if (relatedFor(section).loading) {
                  <div class="p-5 text-sm text-[var(--text-muted)]">Chargement...</div>
                } @else if (relatedFor(section).error) {
                  <div class="p-5 text-sm text-[var(--text-muted)]">
                    {{ relatedFor(section).error }}
                  </div>
                } @else if (relatedFor(section).rows.length) {
                  <div class="overflow-x-auto">
                    <table class="data-table w-full border-collapse text-left text-sm">
                      <thead>
                        <tr>
                          <th scope="col">{{ child.title }}</th>
                          @for (col of relatedColumns(section); track col) {
                            <th scope="col">{{ label(child, col) }}</th>
                          }
                        </tr>
                      </thead>
                      <tbody>
                        @for (
                          childRow of relatedFor(section).rows;
                          track childRow['id'] ?? childRow
                        ) {
                          <tr>
                            <td>
                              @if (childRow['id']) {
                                <a
                                  [routerLink]="['/', child.key, childRow['id']]"
                                  class="font-semibold text-[var(--cameroon-red)] hover:underline"
                                >
                                  {{ childTitle(child, childRow) }}
                                </a>
                              } @else {
                                <span>{{ childTitle(child, childRow) }}</span>
                              }
                            </td>
                            @for (col of relatedColumns(section); track col) {
                              <td>
                                <span [class]="badgeClass(col, childRow[col])">{{
                                  display(col, childRow[col])
                                }}</span>
                              </td>
                            }
                          </tr>
                        }
                      </tbody>
                    </table>
                  </div>
                } @else {
                  <div class="p-5 text-sm text-[var(--text-muted)]">Aucun élément lié.</div>
                }
              </section>
            }
          }
          }

          @if (effectiveTab(cfg, item) === 'effectif') {
            <section class="panel overflow-hidden">
              <header class="border-b border-[var(--line-subtle)] p-5">
                <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Effectif</p>
                <h3 class="mt-1 text-xl font-black">Agents affectés à la patrouille</h3>
              </header>
              <app-patrouille-agents-dialog
                [embedded]="true"
                [patrouilleId]="str(item['id'])"
                [communeId]="str(item['commune_id']) || null"
                [patrouilleNom]="str(item['nom'])"
                [patrouilleStatus]="str(item['status'])"
              />
            </section>
          }
        }
      </section>
    } @else {
      <section class="panel p-5">
        <h2 class="text-xl font-black">Détail indisponible</h2>
        <p class="mt-2 text-[var(--text-muted)]">
          La route demandée ne correspond à aucun module détaillé.
        </p>
        <a routerLink="/dashboard" class="btn-primary mt-4">Retour dashboard</a>
      </section>
    }
  `,
})
export class ResourceDetailPage implements OnInit, OnDestroy {
  private readonly route = inject(ActivatedRoute);
  private readonly api = inject(ApiService);
  private readonly lookup = inject(LookupService);
  private subscription?: Subscription;

  protected readonly config = signal<ResourceConfig | null>(null);
  protected readonly row = signal<Row | null>(null);
  protected readonly loading = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly related = signal<Record<string, RelatedState>>({});
  protected readonly activeTab = signal('informations');
  protected readonly avatarUrl = signal<string | null>(null);
  protected readonly uploading = signal(false);
  protected readonly photoError = signal<string | null>(null);

  ngOnInit(): void {
    this.subscription = this.route.paramMap.subscribe((params) => {
      const key = params.get('feature') ?? '';
      const id = params.get('id') ?? '';
      const cfg = resourceConfigs[key] ?? null;
      this.config.set(cfg);
      this.row.set(null);
      this.error.set(null);
      this.related.set({});
      this.activeTab.set('informations');
      this.clearAvatar();
      this.photoError.set(null);
      this.lookup.reset();
      if (!cfg || !id) {
        return;
      }
      this.lookup.loadForConfig(cfg);
      this.load(cfg, id);
    });
  }

  ngOnDestroy(): void {
    this.subscription?.unsubscribe();
    this.clearAvatar();
  }

  /** Charge l'avatar (blob authentifié → object URL) si la fiche en possède un. */
  private loadAvatar(cfg: ResourceConfig, item: Row): void {
    this.clearAvatar();
    const id = item['id'];
    if (!cfg.photoEndpoint || !item['has_photo'] || !id) {
      return;
    }
    this.api.download(cfg.photoEndpoint(String(id))).subscribe({
      next: (blob) => this.avatarUrl.set(URL.createObjectURL(blob)),
      error: () => this.avatarUrl.set(null),
    });
  }

  private clearAvatar(): void {
    const url = this.avatarUrl();
    if (url) {
      URL.revokeObjectURL(url);
    }
    this.avatarUrl.set(null);
  }

  protected onPhotoSelected(event: Event, cfg: ResourceConfig, item: Row): void {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file || !cfg.photoEndpoint || !item['id']) {
      return;
    }
    this.uploading.set(true);
    this.photoError.set(null);
    const form = new FormData();
    form.append('file', file);
    this.api.post<Row>(cfg.photoEndpoint(String(item['id'])), form).subscribe({
      next: (updated) => {
        this.uploading.set(false);
        input.value = '';
        this.row.set(updated);
        this.loadAvatar(cfg, updated);
      },
      error: () => {
        this.uploading.set(false);
        input.value = '';
        this.photoError.set("Échec de l'envoi de la photo. Vérifie le format et la taille.");
      },
    });
  }

  protected title(): string {
    const item = this.row();
    if (!item) {
      return this.config()?.title ?? 'Détail';
    }
    const key = TITLE_KEYS.find((candidate) => item[candidate]);
    return key ? this.display(key, item[key]) : (this.config()?.title ?? 'Détail');
  }

  protected childTitle(child: ResourceConfig, row: Row): string {
    const key = TITLE_KEYS.find((candidate) => row[candidate]);
    return key ? this.display(key, row[key]) : child.title;
  }

  protected summaryFields(cfg: ResourceConfig): string[] {
    return cfg.detail?.summaryFields ?? this.detailFields(cfg).slice(0, 3);
  }

  protected detailFields(cfg: ResourceConfig): string[] {
    return cfg.detailFields ?? [...cfg.columns, ...(cfg.secondaryColumns ?? [])];
  }

  /** Coercition sûre d'une valeur de ligne (typée unknown) en chaîne pour le binding. */
  protected str(value: unknown): string {
    return value === null || value === undefined ? '' : String(value);
  }

  protected relatedSections(cfg: ResourceConfig): RelatedSection[] {
    return cfg.detail?.related ?? [];
  }

  /** Onglets disponibles pour la fiche courante (selon localisation et entités liées). */
  protected tabs(cfg: ResourceConfig, item: Row): { key: string; label: string }[] {
    const tabs = [{ key: 'informations', label: 'Informations' }];
    if (this.hasLocation(item)) {
      tabs.push({ key: 'localisation', label: 'Localisation' });
    }
    if (this.relatedSections(cfg).length) {
      tabs.push({ key: 'liens', label: 'Entités liées' });
    }
    if (cfg.manageAgents) {
      tabs.push({ key: 'effectif', label: 'Effectif' });
    }
    return tabs;
  }

  /** Onglet effectivement affiché : repli sur « Informations » si l'onglet actif n'existe pas. */
  protected effectiveTab(cfg: ResourceConfig, item: Row): string {
    const active = this.activeTab();
    return this.tabs(cfg, item).some((tab) => tab.key === active) ? active : 'informations';
  }

  protected childConfig(section: RelatedSection): ResourceConfig | null {
    return resourceConfigs[section.key] ?? null;
  }

  protected relatedColumns(section: RelatedSection): string[] {
    const child = resourceConfigs[section.key];
    if (!child) {
      return [];
    }
    const base = section.columns ?? child.columns;
    return base
      .filter(
        (col) =>
          col !== 'id' &&
          col !== section.foreignKey &&
          !col.endsWith('_id') &&
          !TITLE_KEYS.includes(col),
      )
      .slice(0, 3);
  }

  protected relatedFor(section: RelatedSection): RelatedState {
    return this.related()[section.key] ?? { loading: false, error: null, rows: [] };
  }

  protected label(cfg: ResourceConfig, field: string): string {
    return cfg.labels[field] ?? field;
  }

  protected display(field: string, value: unknown): string {
    if (value === null || value === undefined || value === '') {
      return '-';
    }
    if (field === 'subject_type') {
      return this.displaySubjectType(value);
    }
    if (field === 'interventions') {
      return this.displayInterventions(value);
    }
    const relation = this.lookup.label(field, value);
    if (relation) {
      return relation;
    }
    if (Array.isArray(value)) {
      return value.join(', ');
    }
    if (typeof value === 'boolean') {
      return value ? 'Oui' : 'Non';
    }
    if (this.isMoneyField(field)) {
      return `${Number(value ?? 0).toLocaleString('fr-FR')} FCFA`;
    }
    if (typeof value === 'string' && value.includes('T') && value.endsWith('Z')) {
      return new Date(value).toLocaleString('fr-FR');
    }
    if (field === 'status') {
      return this.humanStatus(String(value));
    }
    if (typeof value === 'object') {
      return JSON.stringify(value);
    }
    // Jamais d'UUID brut à l'écran : repli neutre si non résolu.
    if (isUuidLike(value)) {
      return '—';
    }
    return String(value);
  }

  protected badgeClass(field: string, value: unknown): string {
    if (field !== 'status' && typeof value !== 'boolean') {
      return '';
    }
    const text = String(value);
    if (value === true || ['ACTIF', 'PAYE', 'TRAITE', 'CLOTUREE', 'NON_PAYANT'].includes(text)) {
      return 'status-badge ok';
    }
    if (['SUSPENDU', 'ANNULE', 'REJETE', 'EN_RETARD', 'RETRAITE'].includes(text)) {
      return 'status-badge danger';
    }
    return 'status-badge warn';
  }

  /** Lien vers la fiche référencée par un audit log, ou null si la table n'est pas mappable. */
  protected entityLink(item: Row): (string | unknown)[] | null {
    const feature = featureForEntityType(item['entity_type']);
    const entityId = item['entity_id'];
    return feature && entityId ? ['/', feature, entityId] : null;
  }

  private load(cfg: ResourceConfig, id: string): void {
    this.loading.set(true);
    this.api.get<Row>(`${cfg.endpoint}/${id}`).subscribe({
      next: (row) => {
        this.row.set(row);
        this.loading.set(false);
        this.loadAvatar(cfg, row);
        this.loadRelated(cfg, id);
      },
      error: () => {
        this.error.set(
          'Chargement du détail impossible. Vérifie les droits ou la disponibilité API.',
        );
        this.loading.set(false);
      },
    });
  }

  private loadRelated(cfg: ResourceConfig, id: string): void {
    for (const section of this.relatedSections(cfg)) {
      const child = resourceConfigs[section.key];
      if (!child) {
        continue;
      }
      this.setRelated(section.key, { loading: true, error: null, rows: [] });
      this.api.page<Row>(child.endpoint, { [section.foreignKey]: id, page_size: 50 }).subscribe({
        next: (response) => {
          // Filtrage client de sécurité : tous les endpoints n'appliquent pas le filtre serveur.
          const rows = response.items
            .filter((item) => String(item[section.foreignKey] ?? '') === String(id))
            .slice(0, 10);
          this.setRelated(section.key, { loading: false, error: null, rows });
        },
        error: () => {
          this.setRelated(section.key, {
            loading: false,
            error: 'Chargement des éléments liés impossible.',
            rows: [],
          });
        },
      });
    }
  }

  private setRelated(key: string, state: RelatedState): void {
    this.related.update((current) => ({ ...current, [key]: state }));
  }

  protected hasLocation(item: Row): boolean {
    return this.pointLat(item) != null || this.boundaryGeometry(item) != null;
  }

  protected pointLat(item: Row): number | null {
    return this.toNumber(item['gps_latitude']);
  }

  protected pointLon(item: Row): number | null {
    return this.toNumber(item['gps_longitude']);
  }

  protected boundaryGeometry(item: Row): GeoGeometry | null {
    const value = item['boundary'];
    return value && typeof value === 'object' ? (value as GeoGeometry) : null;
  }

  protected mapLayer(cfg: ResourceConfig): string {
    return cfg.key === 'communes' ? 'communes' : cfg.key === 'zones' ? 'zones' : 'pvs';
  }

  private toNumber(value: unknown): number | null {
    return value === null || value === undefined || value === '' ? null : Number(value);
  }

  private isMoneyField(field: string): boolean {
    return field.endsWith('_fcfa') || field.includes('montant') || field.includes('amount_');
  }

  private humanStatus(value: string): string {
    const knownLabel = STATUS_LABELS[value];
    if (knownLabel) {
      return knownLabel;
    }
    return value
      .toLowerCase()
      .split('_')
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(' ');
  }

  private displaySubjectType(value: unknown): string {
    switch (String(value)) {
      case 'PERSON_ONLY':
        return 'Usager sans véhicule';
      case 'VEHICLE_ONLY':
        return 'Véhicule sans conducteur';
      case 'PERSON_WITH_VEHICLE':
        return 'Usager avec véhicule';
      default:
        return String(value);
    }
  }

  private displayInterventions(value: unknown): string {
    if (!Array.isArray(value)) {
      return String(value);
    }
    const labels = value
      .map((item) => {
        if (!this.isRecord(item)) {
          return String(item);
        }
        const name = String(item['nom'] ?? item['intervention_id'] ?? '').trim();
        const amount = Number(item['montant_fcfa'] ?? 0);
        return amount > 0 ? `${name} (${amount.toLocaleString('fr-FR')} FCFA)` : name;
      })
      .filter(Boolean);
    return labels.length ? labels.join(', ') : '-';
  }

  private isRecord(value: unknown): value is Row {
    return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
  }
}
