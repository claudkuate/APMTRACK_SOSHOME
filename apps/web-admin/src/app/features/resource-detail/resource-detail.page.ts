import { Component, OnDestroy, OnInit, inject, signal } from '@angular/core';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { Subscription } from 'rxjs';

import { ApiService } from '../../core/services/api.service';
import { ResourceConfig, resourceConfigs } from '../../shared/resource-config';

type Row = Record<string, unknown>;

@Component({
  selector: 'app-resource-detail-page',
  imports: [RouterLink],
  template: `
    @if (config(); as cfg) {
      <section class="grid gap-5">
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div>
            <p class="text-xs font-bold uppercase text-[var(--text-muted)]">{{ cfg.title }}</p>
            <h2 class="text-2xl font-black">{{ title() }}</h2>
            <p class="mt-1 max-w-3xl text-sm text-[var(--text-muted)]">{{ cfg.description }}</p>
          </div>
          <a [routerLink]="['/', cfg.key]" class="btn-secondary">Retour liste</a>
        </div>

        @if (error()) {
          <div class="panel border-red-200 bg-red-50 p-3 text-sm font-semibold text-red-800">{{ error() }}</div>
        }

        @if (loading()) {
          <div class="panel p-5 text-[var(--text-muted)]">Chargement du detail...</div>
        } @else if (row(); as item) {
          <section class="panel overflow-hidden">
            <header class="border-b border-[var(--line-subtle)] p-5">
              <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Detail dedie</p>
              <h3 class="mt-1 text-xl font-black">{{ title() }}</h3>
            </header>
            <dl class="grid gap-3 p-5 md:grid-cols-2 xl:grid-cols-3">
              @for (field of detailFields(cfg); track field) {
                <div class="detail-item">
                  <dt>{{ label(cfg, field) }}</dt>
                  <dd>
                    <span [class]="badgeClass(field, item[field])">{{ display(field, item[field]) }}</span>
                  </dd>
                </div>
              }
            </dl>
          </section>
        }
      </section>
    } @else {
      <section class="panel p-5">
        <h2 class="text-xl font-black">Detail indisponible</h2>
        <p class="mt-2 text-[var(--text-muted)]">La route demandee ne correspond a aucun module detaille.</p>
        <a routerLink="/dashboard" class="btn-primary mt-4">Retour dashboard</a>
      </section>
    }
  `,
})
export class ResourceDetailPage implements OnInit, OnDestroy {
  private readonly route = inject(ActivatedRoute);
  private readonly api = inject(ApiService);
  private subscription?: Subscription;

  protected readonly config = signal<ResourceConfig | null>(null);
  protected readonly row = signal<Row | null>(null);
  protected readonly loading = signal(false);
  protected readonly error = signal<string | null>(null);

  ngOnInit(): void {
    this.subscription = this.route.paramMap.subscribe((params) => {
      const key = params.get('feature') ?? '';
      const id = params.get('id') ?? '';
      const cfg = resourceConfigs[key] ?? null;
      this.config.set(cfg);
      this.row.set(null);
      this.error.set(null);
      if (!cfg || !id) {
        return;
      }
      this.load(cfg, id);
    });
  }

  ngOnDestroy(): void {
    this.subscription?.unsubscribe();
  }

  protected title(): string {
    const item = this.row();
    if (!item) {
      return this.config()?.title ?? 'Detail';
    }
    const preferred = ['pv_number', 'signalement_number', 'nom', 'full_name', 'email', 'receipt_number'];
    const key = preferred.find((candidate) => item[candidate]);
    return key ? this.display(key, item[key]) : this.config()?.title ?? 'Detail';
  }

  protected detailFields(cfg: ResourceConfig): string[] {
    return cfg.detailFields ?? [...cfg.columns, ...(cfg.secondaryColumns ?? [])];
  }

  protected label(cfg: ResourceConfig, field: string): string {
    return cfg.labels[field] ?? field;
  }

  protected display(field: string, value: unknown): string {
    if (value === null || value === undefined || value === '') {
      return '-';
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
    if (typeof value === 'object') {
      return JSON.stringify(value);
    }
    if (field === 'status') {
      return this.humanStatus(String(value));
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

  private load(cfg: ResourceConfig, id: string): void {
    this.loading.set(true);
    this.api.get<Row>(`${cfg.endpoint}/${id}`).subscribe({
      next: (row) => {
        this.row.set(row);
        this.loading.set(false);
      },
      error: () => {
        this.error.set('Chargement du detail impossible. Verifie les droits ou la disponibilite API.');
        this.loading.set(false);
      },
    });
  }

  private isMoneyField(field: string): boolean {
    return field.endsWith('_fcfa') || field.includes('montant') || field.includes('amount_');
  }

  private humanStatus(value: string): string {
    return value
      .toLowerCase()
      .split('_')
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(' ');
  }
}
