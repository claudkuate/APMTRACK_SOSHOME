import { Component, OnInit, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { ApiService } from '../../core/services/api.service';
import { CommuneContextService } from '../../core/services/commune-context.service';
import { Paginated } from '../../shared/api-types';
import { downloadCsv } from '../../shared/csv';
import { describeHttpError } from '../../shared/http-error';

interface PendingPv {
  pv_id: string;
  pv_number: string;
  commune_id: string;
  amount_due_fcfa: number | null;
  amount_penalty_fcfa: number;
  amount_total_fcfa: number | null;
  due_date: string;
  created_at: string;
}

interface Payment {
  id: string;
  pv_id: string;
  receipt_number: string | null;
  amount_paid_fcfa: number | null;
  amount_total_fcfa: number | null;
  status: string;
  paid_at: string | null;
}

type PayMode = 'ESPECES' | 'MOMO' | 'OM';

@Component({
  selector: 'app-payments-page',
  imports: [FormsModule],
  template: `
    <section class="grid gap-5">
      <!-- Header -->
      <div class="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p class="text-xs font-bold uppercase tracking-wide text-[var(--cameroon-green-strong)]">
            Pilotage › Caisse &amp; paiements
          </p>
          <h2 class="mt-1 text-3xl font-bold">Validation des encaissements</h2>
        </div>
        <div class="flex flex-wrap gap-2">
          <button type="button" class="btn-secondary" (click)="load()">Historique</button>
          <button type="button" class="btn-secondary" (click)="exportPayments()">Journal du jour</button>
        </div>
      </div>

      @if (message()) {
        <div
          id="payments-message"
          [class]="
            messageKind() === 'error'
              ? 'panel flex flex-wrap items-center justify-between gap-2 bg-[var(--tint-red)] p-3 text-sm font-semibold text-[var(--red-ink)]'
              : 'panel flex flex-wrap items-center justify-between gap-2 p-3 text-sm font-semibold text-[var(--cameroon-green-strong)]'
          "
        >
          <span>{{ message() }}</span>
          @if (messageKind() === 'success' && lastPayment(); as payment) {
            <button type="button" class="btn-secondary" (click)="receipt(payment)">Télécharger le reçu</button>
          }
        </div>
      }

      <!-- Search -->
      <div class="panel flex flex-wrap items-center gap-2 p-3">
        <div class="topbar-search min-w-[240px] flex-1">
          <span class="topbar-search__icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
              <circle cx="11" cy="11" r="7" /><path d="m21 21-4.3-4.3" />
            </svg>
          </span>
          <label class="sr-only" for="pv-search">Rechercher un PV</label>
          <input id="pv-search" [(ngModel)]="pendingSearch" placeholder="N° PV, plaque, montant, date..." />
        </div>
        <button type="button" class="btn-secondary" disabled title="Bientôt disponible">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="7" height="7" rx="1" /><rect x="14" y="3" width="7" height="7" rx="1" /><rect x="3" y="14" width="7" height="7" rx="1" /><path d="M14 14h3v3M21 21v.01M17 21v.01M21 17v.01" />
          </svg>
          Scanner le QR
        </button>
      </div>

      <div class="grid gap-4 lg:grid-cols-[1.7fr_1fr]">
        <!-- Left: detail or list -->
        <div class="grid gap-4">
          @if (selected(); as pv) {
            <section class="section-card p-5">
              <div class="flex flex-wrap items-start justify-between gap-3">
                <div class="flex items-center gap-3">
                  <span class="kpi-icon">
                    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                      <path d="M14 3v4a1 1 0 0 0 1 1h4" /><path d="M17 21H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7l5 5v11a2 2 0 0 1-2 2z" />
                    </svg>
                  </span>
                  <div>
                    <strong class="num block text-lg">{{ pv.pv_number }}</strong>
                    <span class="text-xs text-[var(--text-muted)]">Émis le {{ date(pv.created_at) }}</span>
                  </div>
                </div>
                <span [class]="isLate(pv) ? 'status-badge danger' : 'status-badge warn'">
                  {{ isLate(pv) ? 'En retard' : 'En attente' }}
                </span>
              </div>

              <div class="my-4 rounded-xl bg-[var(--surface-muted)] p-4">
                <div class="money-row">
                  <span class="text-[var(--text-body)]">Montant initial</span>
                  <span class="num font-bold">{{ fcfa(pv.amount_due_fcfa) }}</span>
                </div>
                @if (pv.amount_penalty_fcfa > 0) {
                  <div class="money-row text-[var(--red-ink)]">
                    <span>Pénalité de retard — échéance {{ date(pv.due_date) }}</span>
                    <span class="num font-bold">{{ fcfa(pv.amount_penalty_fcfa) }}</span>
                  </div>
                }
                <div class="money-row is-total">
                  <span class="font-bold">Total à encaisser</span>
                  <span class="num text-2xl font-bold text-[var(--cameroon-green-strong)]">{{ fcfa(pv.amount_total_fcfa) }}</span>
                </div>
              </div>

              <div class="grid gap-4 sm:grid-cols-2">
                <div class="field">
                  <label for="payment-amount">Montant encaissé</label>
                  <input
                    id="payment-amount"
                    type="number"
                    readonly
                    class="cursor-default bg-[var(--surface-muted)]"
                    [ngModel]="confirmAmount()"
                  />
                </div>
                <div>
                  <p class="text-[0.78rem] font-bold text-[var(--text-muted)]">Mode de règlement</p>
                  <div class="mt-1 flex flex-wrap gap-2">
                    @for (mode of payModes; track mode.value) {
                      <button
                        type="button"
                        class="chip"
                        [class.is-active]="payMode() === mode.value"
                        [class.opacity-50]="!mode.enabled"
                        [disabled]="!mode.enabled"
                        [title]="mode.enabled ? '' : 'Bientôt disponible'"
                        (click)="mode.enabled && payMode.set(mode.value)"
                      >
                        {{ mode.label }}
                      </button>
                    }
                  </div>
                </div>
              </div>

              <div class="mt-5 flex flex-wrap gap-2">
                <button type="button" class="btn-primary flex-1" [disabled]="isValidating()" (click)="confirmValidate()">
                  <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M20 6 9 17l-5-5" />
                  </svg>
                  {{ isValidating() ? 'Validation en cours…' : 'Valider le paiement' }}
                </button>
                <button type="button" class="btn-secondary" [disabled]="isValidating()" (click)="clearSelection()">Annuler</button>
              </div>

              @if (messageKind() === 'error' && message(); as errorMessage) {
                <p class="mt-3 rounded-lg bg-[var(--tint-red)] p-3 text-sm font-semibold text-[var(--red-ink)]">
                  {{ errorMessage }}
                </p>
              }

              <p class="mt-3 flex items-center gap-2 text-xs text-[var(--text-muted)]">
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z" />
                </svg>
                Le montant et la pénalité sont calculés par le serveur. Le receveur ne peut pas les modifier.
              </p>
            </section>
          } @else {
            <section class="section-card overflow-hidden">
              <div class="section-head">
                <h3>PV à encaisser</h3>
                <span class="status-badge warn">{{ filteredPending().length }} visible(s)</span>
              </div>
              <div class="overflow-x-auto">
                <table class="data-table w-full min-w-[620px] text-left text-sm">
                  <thead>
                    <tr>
                      <th>N° PV</th>
                      <th>Montant</th>
                      <th>Pénalité</th>
                      <th>Total</th>
                      <th>Échéance</th>
                    </tr>
                  </thead>
                  <tbody>
                    @for (pv of filteredPending(); track pv.pv_id) {
                      <tr class="cursor-pointer" (click)="selectPv(pv)">
                        <td class="num font-semibold">{{ pv.pv_number }}</td>
                        <td class="num">{{ fcfa(pv.amount_due_fcfa) }}</td>
                        <td class="num text-[var(--red-ink)]">{{ pv.amount_penalty_fcfa ? fcfa(pv.amount_penalty_fcfa) : '—' }}</td>
                        <td class="num font-bold">{{ fcfa(pv.amount_total_fcfa) }}</td>
                        <td class="text-[var(--text-muted)]">{{ date(pv.due_date) }}</td>
                      </tr>
                    } @empty {
                      <tr><td colspan="5" class="py-7 text-center text-[var(--text-muted)]">Aucun PV en attente.</td></tr>
                    }
                  </tbody>
                </table>
              </div>
            </section>
          }
        </div>

        <!-- Right aside -->
        <aside class="grid content-start gap-4">
          <section class="section-card p-5">
            <div class="flex items-center justify-between">
              <h3 class="font-serif text-lg font-bold">Caisse du jour</h3>
              <span class="text-xs text-[var(--text-muted)]">{{ todayLabel() }}</span>
            </div>
            <div class="mt-4 grid gap-3 sm:grid-cols-2">
              <div class="rounded-xl bg-[var(--cameroon-green)] p-4 text-white">
                <p class="text-xs font-semibold opacity-90">Total encaissé</p>
                <strong class="num mt-1 block text-2xl font-bold">{{ fcfaShort(totalToday()) }}</strong>
                <p class="mt-1 text-xs opacity-90">{{ countToday() }} reçus</p>
              </div>
              <div class="rounded-xl bg-[var(--tint-red)] p-4 text-[var(--red-ink)]">
                <p class="text-xs font-semibold">Pénalités en attente</p>
                <strong class="num mt-1 block text-2xl font-bold">{{ fcfaShort(pendingPenalty()) }}</strong>
                <p class="mt-1 text-xs">{{ pendingLateCount() }} PV en retard</p>
              </div>
            </div>
          </section>

          <section class="section-card overflow-hidden">
            <div class="section-head"><h3>Encaissements récents</h3></div>
            <div class="overflow-x-auto">
              <table class="data-table w-full min-w-[320px] text-left text-sm">
                <thead>
                  <tr><th>Reçu</th><th>Montant</th><th>Date</th><th></th></tr>
                </thead>
                <tbody>
                  @for (payment of payments(); track payment.id) {
                    <tr>
                      <td class="num font-semibold">{{ payment.receipt_number ?? '—' }}</td>
                      <td class="num">{{ fcfa(payment.amount_paid_fcfa) }}</td>
                      <td class="text-[var(--text-muted)]">{{ time(payment.paid_at) }}</td>
                      <td class="text-right">
                        <button type="button" class="btn-ghost min-h-8 px-2 text-xs" (click)="receipt(payment)">Reçu</button>
                      </td>
                    </tr>
                  } @empty {
                    <tr><td colspan="4" class="py-7 text-center text-[var(--text-muted)]">Aucun paiement.</td></tr>
                  }
                </tbody>
              </table>
            </div>
          </section>
        </aside>
      </div>
    </section>
  `,
})
export class PaymentsPage implements OnInit {
  private readonly api = inject(ApiService);
  private readonly commune = inject(CommuneContextService);

  protected readonly pending = signal<PendingPv[]>([]);
  protected readonly payments = signal<Payment[]>([]);
  protected readonly message = signal<string | null>(null);
  protected readonly messageKind = signal<'success' | 'error'>('success');
  protected readonly selected = signal<PendingPv | null>(null);
  protected readonly confirmAmount = signal(0);
  protected readonly payMode = signal<PayMode>('ESPECES');
  protected readonly isValidating = signal(false);
  protected readonly lastPayment = signal<Payment | null>(null);
  protected pendingSearch = '';

  // Phase pilote : encaissement en espèces uniquement — Mobile Money viendra
  // avec l'intégration des API de paiement, commune par commune.
  protected readonly payModes: { value: PayMode; label: string; enabled: boolean }[] = [
    { value: 'ESPECES', label: 'Espèces', enabled: true },
    { value: 'MOMO', label: 'MTN MoMo', enabled: false },
    { value: 'OM', label: 'Orange Money', enabled: false },
  ];

  protected readonly totalToday = computed(() =>
    this.todayPayments().reduce((sum, payment) => sum + Number(payment.amount_paid_fcfa ?? 0), 0),
  );
  protected readonly countToday = computed(() => this.todayPayments().length);
  protected readonly pendingPenalty = computed(() =>
    this.pending().reduce((sum, pv) => sum + Number(pv.amount_penalty_fcfa ?? 0), 0),
  );
  protected readonly pendingLateCount = computed(
    () => this.pending().filter((pv) => Number(pv.amount_penalty_fcfa ?? 0) > 0).length,
  );

  ngOnInit(): void {
    this.load();
  }

  protected load(): void {
    const communeId = this.commune.communeId();
    const scope = communeId ? { commune_id: communeId } : {};
    this.api.page<PendingPv>('/api/v1/payments/pending', { page_size: 50, ...scope }).subscribe({
      next: (response: Paginated<PendingPv>) => {
        this.pending.set(response.items);
        // Resynchronise la fiche ouverte : montants/pénalités recalculés par le
        // serveur, ou fermeture si le PV n'est plus en attente.
        const current = this.selected();
        if (current) {
          const refreshed = response.items.find((pv) => pv.pv_id === current.pv_id) ?? null;
          this.selected.set(refreshed);
          if (refreshed) {
            this.confirmAmount.set(Number(refreshed.amount_total_fcfa ?? 0));
          }
        }
      },
      error: (err: unknown) => this.notify('error', describeHttpError(err, 'Chargement des PV en attente')),
    });
    this.api.page<Payment>('/api/v1/payments', { page_size: 50, ...scope }).subscribe({
      next: (response: Paginated<Payment>) => this.payments.set(response.items),
      error: (err: unknown) => this.notify('error', describeHttpError(err, "Chargement de l'historique")),
    });
  }

  protected selectPv(pv: PendingPv): void {
    this.selected.set(pv);
    this.confirmAmount.set(Number(pv.amount_total_fcfa ?? 0));
    this.payMode.set('ESPECES');
  }

  protected clearSelection(): void {
    this.selected.set(null);
  }

  protected isLate(pv: PendingPv): boolean {
    return Number(pv.amount_penalty_fcfa ?? 0) > 0;
  }

  protected confirmValidate(): void {
    const pv = this.selected();
    if (!pv || this.isValidating()) {
      return;
    }
    this.isValidating.set(true);
    this.message.set(null);
    this.lastPayment.set(null);
    const amount = this.confirmAmount();
    this.api
      .post<Payment>(`/api/v1/payments/${pv.pv_id}/validate`, { amount_paid_fcfa: amount })
      .subscribe({
        next: (payment) => {
          this.isValidating.set(false);
          this.lastPayment.set(payment);
          this.notify('success', `Paiement validé — reçu ${payment.receipt_number ?? payment.id}.`);
          this.clearSelection();
          this.load();
          this.commune.refreshCounters();
          this.scrollMessageIntoView();
        },
        error: (err: unknown) => {
          this.isValidating.set(false);
          this.notify('error', describeHttpError(err, 'Validation du paiement'));
          // Recharge les montants : une pénalité a pu apparaître depuis l'affichage.
          this.load();
        },
      });
  }

  private notify(kind: 'success' | 'error', text: string): void {
    this.messageKind.set(kind);
    this.message.set(text);
  }

  private scrollMessageIntoView(): void {
    setTimeout(() => {
      document.getElementById('payments-message')?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    });
  }

  protected filteredPending(): PendingPv[] {
    const query = this.pendingSearch.trim().toLowerCase();
    if (!query) {
      return this.pending();
    }
    return this.pending().filter((pv) => JSON.stringify(pv).toLowerCase().includes(query));
  }

  private todayPayments(): Payment[] {
    const today = new Date().toDateString();
    return this.payments().filter((payment) => payment.paid_at && new Date(payment.paid_at).toDateString() === today);
  }

  protected receipt(payment: Payment): void {
    this.api.openDownload(
      `/api/v1/payments/${payment.id}/receipt`,
      `${payment.receipt_number ?? payment.id}.pdf`,
      undefined,
      (err) => this.notify('error', describeHttpError(err, 'Téléchargement du reçu')),
    );
  }

  protected exportPayments(): void {
    const rows: Record<string, string>[] = this.payments().map((payment) => ({
      recu: payment.receipt_number ?? payment.id,
      pv: payment.pv_id,
      montant: this.fcfa(payment.amount_paid_fcfa),
      total: this.fcfa(payment.amount_total_fcfa),
      statut: payment.status,
      date: this.date(payment.paid_at),
    }));
    if (!rows.length) {
      this.notify('error', 'Aucun paiement à exporter.');
      return;
    }
    const columns = Array.from(new Set(rows.flatMap((row) => Object.keys(row))));
    downloadCsv('journal-du-jour.csv', [
      columns,
      ...rows.map((row) => columns.map((column) => row[column] ?? '')),
    ]);
  }

  protected fcfa(value: number | null | undefined): string {
    return `${Number(value ?? 0).toLocaleString('fr-FR')} FCFA`;
  }

  protected fcfaShort(value: number | null | undefined): string {
    const amount = Number(value ?? 0);
    if (amount >= 1_000_000) {
      return `${(amount / 1_000_000).toLocaleString('fr-FR', { maximumFractionDigits: 2 })} M`;
    }
    if (amount >= 1_000) {
      return `${(amount / 1_000).toLocaleString('fr-FR', { maximumFractionDigits: 0 })} K`;
    }
    return amount.toLocaleString('fr-FR');
  }

  protected date(value: string | null): string {
    return value ? new Date(value).toLocaleDateString('fr-FR', { day: '2-digit', month: 'short', year: 'numeric' }) : '—';
  }

  protected time(value: string | null): string {
    return value ? new Date(value).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' }) : '—';
  }

  protected todayLabel(): string {
    return new Date().toLocaleDateString('fr-FR', { day: 'numeric', month: 'long' });
  }

}
