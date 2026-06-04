import { Component, OnInit, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { ApiService } from '../../core/services/api.service';
import { LookupOption, Paginated } from '../../shared/api-types';

type Row = Record<string, unknown>;

@Component({
  selector: 'app-exports-page',
  imports: [FormsModule],
  template: `
    <section class="grid gap-5">
      <div>
        <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Exports</p>
        <h2 class="text-2xl font-black">Exports CSV</h2>
        <p class="mt-1 max-w-3xl text-sm text-[var(--text-muted)]">
          Les exports utilisent les permissions backend. Les filtres ci-dessous s'appliquent aux fichiers telecharges.
        </p>
      </div>

      <section class="panel grid gap-4 p-4 md:grid-cols-4">
        <div class="field">
          <label>Commune</label>
          <select [(ngModel)]="communeId">
            <option value="">Toutes accessibles</option>
            @for (commune of communes(); track commune.id) {
              <option [value]="commune.id">{{ optionLabel(commune) }}</option>
            }
          </select>
        </div>
        <div class="field">
          <label>Statut</label>
          <select [(ngModel)]="status">
            <option value="">Tous statuts</option>
            @for (item of statuses; track item.value) {
              <option [value]="item.value">{{ item.label }}</option>
            }
          </select>
        </div>
        <div class="field">
          <label>Du</label>
          <input type="date" [(ngModel)]="from" />
        </div>
        <div class="field">
          <label>Au</label>
          <input type="date" [(ngModel)]="to" />
        </div>
      </section>

      <div class="panel p-3 text-sm text-[var(--text-muted)]">
        <strong class="text-[var(--text-strong)]">Filtres actifs:</strong>
        {{ exportSummary() }}
      </div>

      <section class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        @for (exportItem of exports; track exportItem.path) {
          <article class="panel p-4">
            <h3 class="font-black">{{ exportItem.label }}</h3>
            <p class="mt-1 text-sm text-[var(--text-muted)]">{{ exportItem.description }}</p>
            <button type="button" class="btn-primary mt-4" (click)="download(exportItem.path, exportItem.file)">
              Telecharger
            </button>
          </article>
        }
      </section>
    </section>
  `,
})
export class ExportsPage implements OnInit {
  private readonly api = inject(ApiService);

  protected readonly communes = signal<LookupOption[]>([]);
  protected communeId = '';
  protected status = '';
  protected from = '';
  protected to = '';
  protected readonly statuses = [
    { value: 'ACTIF', label: 'Actif' },
    { value: 'SUSPENDU', label: 'Suspendu' },
    { value: 'EN_ATTENTE_PAIEMENT', label: 'PV en attente paiement' },
    { value: 'PAYE', label: 'Paye' },
    { value: 'EN_RETARD', label: 'En retard' },
    { value: 'RECU', label: 'Signalement recu' },
    { value: 'EN_COURS', label: 'En cours' },
    { value: 'TRAITE', label: 'Traite' },
    { value: 'REJETE', label: 'Rejete' },
  ];
  protected readonly exports = [
    { label: 'PV', description: 'Proces-verbaux, statuts et montants initiaux.', path: '/api/v1/exports/pvs', file: 'pvs.csv' },
    { label: 'Paiements', description: 'Journal de caisse, recus et montants encaisses.', path: '/api/v1/exports/payments', file: 'paiements.csv' },
    { label: 'Signalements', description: 'Signalements citoyens et statuts de traitement.', path: '/api/v1/exports/signalements', file: 'signalements.csv' },
    { label: 'Agents', description: 'Agents, grades et statuts operationnels.', path: '/api/v1/exports/agents', file: 'agents.csv' },
  ];

  ngOnInit(): void {
    this.api.page<Row>('/api/v1/communes', { page_size: 100 }).subscribe({
      next: (page: Paginated<Row>) => {
        this.communes.set(
          page.items.map((row) => ({
            id: String(row['id'] ?? ''),
            label: String(row['nom'] ?? row['id'] ?? ''),
            meta: String(row['code'] ?? ''),
          })),
        );
      },
      error: () => this.communes.set([]),
    });
  }

  protected download(path: string, filename: string): void {
    this.api.openDownload(path, filename, {
      commune_id: this.communeId.trim(),
      status: this.status.trim(),
      from: this.from,
      to: this.to,
    });
  }

  protected optionLabel(option: LookupOption): string {
    return option.meta ? `${option.label} - ${option.meta}` : option.label;
  }

  protected exportSummary(): string {
    const parts = [
      this.communeId ? `commune ${this.optionLabel(this.communes().find((item) => item.id === this.communeId) ?? { id: '', label: this.communeId })}` : 'toutes les communes accessibles',
      this.status ? `statut ${this.statusLabel(this.status)}` : 'tous statuts',
      this.from ? `du ${this.from}` : '',
      this.to ? `au ${this.to}` : '',
    ].filter(Boolean);
    return parts.join(', ');
  }

  private statusLabel(value: string): string {
    return this.statuses.find((item) => item.value === value)?.label ?? value;
  }
}
