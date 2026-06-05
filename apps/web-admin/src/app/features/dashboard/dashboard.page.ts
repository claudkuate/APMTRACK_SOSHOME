import { Component, OnInit, computed, inject, signal } from '@angular/core';
import { RouterLink } from '@angular/router';

import { ApiService } from '../../core/services/api.service';
import { CommuneContextService } from '../../core/services/commune-context.service';
import { DashboardSummary, Paginated } from '../../shared/api-types';

interface PvRow {
  id: string;
  pv_number: string;
  status: string;
  amount_initial_fcfa: number | null;
  verbalized_name: string | null;
  verbalized_identity_number: string | null;
  vehicle_plate: string | null;
  vehicle_registration_card_number: string | null;
  created_at: string;
}

interface SignalementRow {
  id: string;
  signalement_number: string;
  type_incident: string;
  location_description: string | null;
  status: string;
  created_at: string;
}

interface DistSegment {
  label: string;
  value: number;
  pct: number;
  color: string;
}

interface StatusBar {
  label: string;
  value: number;
  peak: boolean;
}

@Component({
  selector: 'app-dashboard-page',
  imports: [RouterLink],
  template: `
    <section class="grid gap-5">
      <!-- Header -->
      <div class="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p class="text-xs font-bold uppercase tracking-wide text-[var(--cameroon-green-strong)]">
            Pilotage › Tableau de bord
          </p>
          <h2 class="mt-1 text-3xl font-bold">Supervision communale</h2>
          <p class="mt-1 text-sm text-[var(--text-muted)]">
            Vue d'ensemble de l'activité — {{ scopeLabel() }} · {{ today() }}
          </p>
        </div>
        <div class="flex flex-wrap gap-2">
          <a routerLink="/exports" class="btn-secondary">
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><path d="m7 10 5 5 5-5" /><path d="M12 15V3" />
            </svg>
            Exporter
          </a>
          <a routerLink="/pvs" class="btn-primary">
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <path d="M14 3v4a1 1 0 0 0 1 1h4" /><path d="M17 21H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7l5 5v11a2 2 0 0 1-2 2z" /><path d="M12 11v6M9 14h6" />
            </svg>
            Nouveau PV
          </a>
        </div>
      </div>

      @if (loading()) {
        <div class="panel p-6 text-[var(--text-muted)]">Chargement des indicateurs...</div>
      } @else if (summary()) {
        <!-- KPI cards -->
        <div class="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
          <article class="kpi-card">
            <div class="flex items-start justify-between">
              <span class="kpi-icon">
                <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M14 3v4a1 1 0 0 0 1 1h4" /><path d="M17 21H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7l5 5v11a2 2 0 0 1-2 2z" />
                </svg>
              </span>
            </div>
            <p class="kpi-label mt-3">PV émis (total)</p>
            <strong class="kpi-value mt-1 block">{{ count(pvs()['total']) }}</strong>
            <p class="mt-2 text-xs text-[var(--text-muted)]">{{ count(pvs()['payes']) }} payés</p>
          </article>

          <article class="kpi-card">
            <span class="kpi-icon gold">
              <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                <rect x="2" y="5" width="20" height="14" rx="2" /><path d="M2 10h20" />
              </svg>
            </span>
            <p class="kpi-label mt-3">Encaissé (validé)</p>
            <strong class="kpi-value mt-1 block">{{ fcfaShort(payments()['total_collected_fcfa']) }}</strong>
            <p class="mt-2 text-xs text-[var(--text-muted)]">{{ fcfaShort(payments()['pending_fcfa']) }} en attente</p>
          </article>

          <article class="kpi-card">
            <span class="kpi-icon red">
              <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" />
              </svg>
            </span>
            <p class="kpi-label mt-3">PV en attente de paiement</p>
            <strong class="kpi-value mt-1 block">{{ count(pvs()['en_attente']) }}</strong>
            <p class="mt-2 text-xs font-semibold text-[var(--red-ink)]">{{ count(pvs()['en_retard']) }} en retard</p>
          </article>

          <article class="kpi-card">
            <span class="kpi-icon ink">
              <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="9" cy="8" r="3" /><path d="M3 20a6 6 0 0 1 12 0" /><path d="M16 5a3 3 0 0 1 0 6" />
              </svg>
            </span>
            <p class="kpi-label mt-3">Agents en service</p>
            <strong class="kpi-value mt-1 block">{{ count(agents()['actifs']) }} / {{ count(agents()['total']) }}</strong>
            <p class="mt-2 text-xs text-[var(--text-muted)]">{{ count(agents()['suspendus']) }} suspendus</p>
          </article>
        </div>

        <!-- Charts row -->
        <div class="grid gap-4 lg:grid-cols-[1.6fr_1fr]">
          <section class="section-card">
            <div class="section-head">
              <h3>Procès-verbaux par statut</h3>
              <span class="num text-sm font-semibold text-[var(--text-muted)]">{{ count(pvs()['total']) }} total</span>
            </div>
            <div class="px-5 pb-6">
              @if (statusBars().length) {
                <div class="bar-chart">
                  @for (bar of statusBars(); track bar.label) {
                    <div class="bar-col">
                      <div class="grid h-full content-end gap-2">
                        <span class="bar-value">{{ bar.value }}</span>
                        <span class="bar" [class.is-peak]="bar.peak" [style.height.%]="barHeight(bar.value)"></span>
                      </div>
                      <span class="bar-label">{{ bar.label }}</span>
                    </div>
                  }
                </div>
              } @else {
                <p class="py-8 text-center text-sm text-[var(--text-muted)]">Aucun PV pour ce périmètre.</p>
              }
            </div>
          </section>

          <section class="section-card">
            <div class="section-head"><h3>Répartition des PV</h3></div>
            <div class="px-5 pb-5">
              <div class="dist-track">
                @for (seg of distribution(); track seg.label) {
                  <span class="dist-seg" [style.width.%]="seg.pct" [style.background]="seg.color"></span>
                }
              </div>
              <div class="mt-3">
                @for (seg of distribution(); track seg.label) {
                  <div class="dist-row">
                    <span class="dist-dot" [style.background]="seg.color"></span>
                    <span>{{ seg.label }}</span>
                    <span class="dist-pct">{{ seg.pct }} %</span>
                  </div>
                } @empty {
                  <p class="py-4 text-center text-sm text-[var(--text-muted)]">Aucune donnée.</p>
                }
              </div>
            </div>
          </section>
        </div>

        <!-- Lists row -->
        <div class="grid gap-4 lg:grid-cols-[1.6fr_1fr]">
          <section class="section-card overflow-hidden">
            <div class="section-head">
              <h3>Procès-verbaux récents</h3>
              <a routerLink="/pvs" class="text-sm font-bold text-[var(--cameroon-green-strong)]">Tout voir</a>
            </div>
            <div class="overflow-x-auto">
              <table class="data-table w-full min-w-[620px] text-left text-sm">
                <thead>
                  <tr>
                    <th>N° PV</th>
                    <th>Verbalisé</th>
                    <th>Montant</th>
                    <th>Date</th>
                    <th>Statut</th>
                  </tr>
                </thead>
                <tbody>
                  @for (pv of recentPvs(); track pv.id) {
                    <tr>
                      <td class="num font-semibold">{{ pv.pv_number }}</td>
                      <td>
                        {{
                          pv.verbalized_name ||
                            pv.vehicle_plate ||
                            pv.vehicle_registration_card_number ||
                            pv.verbalized_identity_number ||
                            '—'
                        }}
                      </td>
                      <td class="num">{{ pv.amount_initial_fcfa ? fcfa(pv.amount_initial_fcfa) : '—' }}</td>
                      <td class="text-[var(--text-muted)]">{{ date(pv.created_at) }}</td>
                      <td><span [class]="badgeClass(pv.status)">{{ humanStatus(pv.status) }}</span></td>
                    </tr>
                  } @empty {
                    <tr><td colspan="5" class="py-7 text-center text-[var(--text-muted)]">Aucun PV récent.</td></tr>
                  }
                </tbody>
              </table>
            </div>
          </section>

          <section class="section-card overflow-hidden">
            <div class="section-head">
              <h3>Signalements citoyens</h3>
              @if (summary()) {
                <span class="status-badge danger">{{ count(signalements()['recu']) }} reçus</span>
              }
            </div>
            <div class="grid gap-1 p-3">
              @for (item of recentSignalements(); track item.id) {
                <a routerLink="/signalements" class="search-result">
                  <span class="kpi-icon" aria-hidden="true">
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                      <path d="M4 21V4a1 1 0 0 1 1-1h11l-2 4 2 4H5" />
                    </svg>
                  </span>
                  <span class="min-w-0">
                    <strong class="block truncate">{{ item.type_incident }}</strong>
                    <small class="block truncate text-[var(--text-muted)]">
                      {{ item.location_description || '—' }} · {{ date(item.created_at) }}
                    </small>
                  </span>
                  <span [class]="badgeClass(item.status)">{{ humanStatus(item.status) }}</span>
                </a>
              } @empty {
                <p class="p-4 text-center text-sm text-[var(--text-muted)]">Aucun signalement reçu.</p>
              }
            </div>
          </section>
        </div>
      } @else {
        <div class="panel p-6 text-[var(--cameroon-red)]">Impossible de charger le tableau de bord.</div>
      }
    </section>
  `,
})
export class DashboardPage implements OnInit {
  private readonly api = inject(ApiService);
  private readonly commune = inject(CommuneContextService);

  protected readonly loading = signal(true);
  protected readonly summary = signal<DashboardSummary | null>(null);
  protected readonly recentPvs = signal<PvRow[]>([]);
  protected readonly recentSignalements = signal<SignalementRow[]>([]);

  protected readonly pvs = computed(() => this.summary()?.pvs ?? {});
  protected readonly payments = computed(() => this.summary()?.payments ?? {});
  protected readonly agents = computed(() => this.summary()?.agents ?? {});
  protected readonly signalements = computed(() => this.summary()?.signalements ?? {});

  protected readonly distribution = computed<DistSegment[]>(() => {
    const pvs = this.pvs();
    const total = Number(pvs['total'] ?? 0);
    if (!total) {
      return [];
    }
    const segments = [
      { label: 'Payé', value: Number(pvs['payes'] ?? 0), color: 'var(--cameroon-green)' },
      { label: 'En attente', value: Number(pvs['en_attente'] ?? 0), color: 'var(--cameroon-gold)' },
      { label: 'En retard', value: Number(pvs['en_retard'] ?? 0), color: 'var(--cameroon-red)' },
      {
        label: 'Annulé / non payant',
        value: Number(pvs['annules'] ?? 0) + Number(pvs['non_payants'] ?? 0),
        color: 'var(--line-strong)',
      },
    ];
    return segments
      .filter((seg) => seg.value > 0)
      .map((seg) => ({ ...seg, pct: Math.round((seg.value / total) * 100) }));
  });

  protected readonly statusBars = computed<StatusBar[]>(() => {
    const pvs = this.pvs();
    const bars = [
      { label: 'Payé', value: Number(pvs['payes'] ?? 0) },
      { label: 'Attente', value: Number(pvs['en_attente'] ?? 0) },
      { label: 'Retard', value: Number(pvs['en_retard'] ?? 0) },
      { label: 'Non pay.', value: Number(pvs['non_payants'] ?? 0) },
      { label: 'Annulé', value: Number(pvs['annules'] ?? 0) },
    ];
    const max = Math.max(...bars.map((bar) => bar.value), 0);
    if (max === 0) {
      return [];
    }
    return bars.map((bar) => ({ ...bar, peak: bar.value === max }));
  });

  ngOnInit(): void {
    this.load();
  }

  protected load(): void {
    this.loading.set(true);
    const communeId = this.commune.communeId();
    const scope = communeId ? { commune_id: communeId } : undefined;

    this.api.get<DashboardSummary>('/api/v1/dashboard/summary', scope).subscribe({
      next: (summary) => {
        this.summary.set(summary);
        this.loading.set(false);
      },
      error: () => {
        this.summary.set(null);
        this.loading.set(false);
      },
    });

    this.api
      .page<PvRow>('/api/v1/pvs', { page_size: 5, ...(scope ?? {}) })
      .subscribe({
        next: (response: Paginated<PvRow>) => this.recentPvs.set(response.items),
        error: () => this.recentPvs.set([]),
      });

    this.api
      .page<SignalementRow>('/api/v1/signalements', { page_size: 4, status: 'RECU', ...(scope ?? {}) })
      .subscribe({
        next: (response: Paginated<SignalementRow>) => this.recentSignalements.set(response.items),
        error: () => this.recentSignalements.set([]),
      });
  }

  protected barHeight(value: number): number {
    const max = Math.max(...this.statusBars().map((bar) => bar.value), 1);
    return Math.max((value / max) * 100, value > 0 ? 6 : 0);
  }

  protected scopeLabel(): string {
    return this.commune.current()?.nom ?? 'Toutes les communes';
  }

  protected today(): string {
    return new Date().toLocaleDateString('fr-FR', { day: 'numeric', month: 'long', year: 'numeric' });
  }

  protected count(value: number | undefined): string {
    return Number(value ?? 0).toLocaleString('fr-FR');
  }

  protected fcfa(value: number | null | undefined): string {
    return `${Number(value ?? 0).toLocaleString('fr-FR')} FCFA`;
  }

  protected fcfaShort(value: number | undefined): string {
    const amount = Number(value ?? 0);
    if (amount >= 1_000_000) {
      return `${(amount / 1_000_000).toLocaleString('fr-FR', { maximumFractionDigits: 2 })} M FCFA`;
    }
    if (amount >= 1_000) {
      return `${(amount / 1_000).toLocaleString('fr-FR', { maximumFractionDigits: 0 })} K FCFA`;
    }
    return `${amount.toLocaleString('fr-FR')} FCFA`;
  }

  protected date(value: string | null): string {
    return value ? new Date(value).toLocaleDateString('fr-FR', { day: '2-digit', month: 'short', year: '2-digit' }) : '—';
  }

  protected badgeClass(status: string): string {
    if (['ACTIF', 'PAYE', 'TRAITE', 'NON_PAYANT', 'CLOTUREE'].includes(status)) {
      return 'status-badge ok';
    }
    if (['SUSPENDU', 'ANNULE', 'REJETE', 'EN_RETARD', 'RETRAITE'].includes(status)) {
      return 'status-badge danger';
    }
    return 'status-badge warn';
  }

  protected humanStatus(value: string): string {
    return value
      .toLowerCase()
      .split('_')
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(' ');
  }
}
