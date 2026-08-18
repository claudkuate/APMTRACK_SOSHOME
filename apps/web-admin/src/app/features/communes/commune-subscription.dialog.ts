import { Component, EventEmitter, Input, OnInit, Output, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';

import { AutoTranslatePipe } from '../../core/i18n/auto-translate.pipe';
import { I18nService } from '../../core/i18n/i18n.service';
import { ApiService } from '../../core/services/api.service';
import { Paginated } from '../../shared/api-types';
import { describeHttpError } from '../../shared/http-error';

interface SubscriptionPayment {
  id: string;
  payment_reference: string;
  amount_fcfa: number;
  paid_at: string;
  period_started_at: string;
  period_expires_at: string;
  confirmed_at: string;
  confirmed_by_user_id: string;
  confirmed_by_full_name?: string | null;
  confirmed_by_email?: string | null;
}

@Component({
  selector: 'app-commune-subscription-dialog',
  imports: [ReactiveFormsModule, AutoTranslatePipe],
  template: `
    <div
      class="modal-backdrop"
      role="dialog"
      aria-modal="true"
      aria-label="Gestion de l'abonnement"
      (keydown.escape)="closed.emit()"
      (click)="closed.emit()"
    >
      <div class="modal-panel modal-panel--wide max-h-[92vh] overflow-y-auto" (click)="$event.stopPropagation()">
        <header class="flex items-start justify-between gap-3 border-b border-[var(--line-subtle)] pb-4">
          <div>
            <p class="text-xs font-bold uppercase text-[var(--text-muted)]">{{ 'Abonnement' | auto }}</p>
            <h3 class="text-lg font-black">{{ communeName }}</h3>
          </div>
          <button type="button" class="btn-ghost" (click)="closed.emit()">{{ 'Fermer' | auto }}</button>
        </header>

        @if (message()) {
          <p class="mt-4 rounded-md border border-green-200 bg-green-50 p-3 text-sm font-semibold text-green-800">
            {{ message() }}
          </p>
        }
        @if (error()) {
          <p class="mt-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm font-semibold text-red-800">
            {{ error() }}
          </p>
        }

        @if (subscriptionActive) {
          <p class="mt-4 rounded-md border border-green-200 bg-green-50 p-3 text-sm font-semibold text-green-800">
            {{ 'Accès effectif actif.' | auto }}
          </p>
        } @else if (subscriptionEntitlementCurrent) {
          <p class="mt-4 rounded-md border border-amber-200 bg-amber-50 p-3 text-sm font-semibold text-amber-900">
            {{ 'Droit d’abonnement confirmé, mais accès actuellement inactif ou non commencé.' | auto }}
          </p>
        }

        <div class="mt-4 flex flex-wrap gap-2">
          <button
            type="button"
            [class]="mode() === 'payment' ? 'btn-primary' : 'btn-secondary'"
            (click)="mode.set('payment')"
          >
            {{ 'Confirmer un paiement' | auto }}
          </button>
          <button
            type="button"
            [class]="mode() === 'trial' ? 'btn-primary' : 'btn-secondary'"
            [disabled]="subscriptionEntitlementCurrent"
            (click)="mode.set('trial')"
          >
            {{ 'Ouvrir une période d’essai' | auto }}
          </button>
        </div>

        @if (mode() === 'payment') {
          <form class="mt-5 grid gap-4" [formGroup]="paymentForm" (ngSubmit)="confirmPayment()">
            <div class="grid gap-4 md:grid-cols-2">
              <div class="field">
                <label for="subscription-reference">{{ 'Référence du paiement' | auto }} *</label>
                <input id="subscription-reference" type="text" formControlName="payment_reference" maxlength="160" />
              </div>
              <div class="field">
                <label for="subscription-amount">{{ 'Montant payé (FCFA)' | auto }} *</label>
                <input id="subscription-amount" type="number" min="1" step="1" formControlName="amount_fcfa" />
              </div>
              <div class="field">
                <label for="subscription-paid-at">{{ 'Date du paiement' | auto }} *</label>
                <input id="subscription-paid-at" type="datetime-local" step="1" formControlName="paid_at" />
              </div>
              <div class="field">
                <label for="subscription-start">{{ 'Début de la période' | auto }} *</label>
                <input
                  id="subscription-start"
                  type="datetime-local"
                  step="1"
                  formControlName="period_started_at"
                  [readOnly]="subscriptionEntitlementCurrent"
                />
                @if (subscriptionEntitlementCurrent) {
                  <p class="field-help">{{ 'Le renouvellement commence à l’échéance actuelle.' | auto }}</p>
                }
              </div>
              <div class="field">
                <label for="subscription-end">{{ 'Fin de la période' | auto }} *</label>
                <input id="subscription-end" type="datetime-local" step="1" formControlName="period_expires_at" />
              </div>
            </div>
            <div class="flex justify-end">
              <button type="submit" class="btn-primary" [disabled]="paymentForm.invalid || saving()">
                {{ (saving() ? 'Confirmation...' : 'Confirmer et activer') | auto }}
              </button>
            </div>
          </form>
        } @else {
          <form class="mt-5 grid gap-4" [formGroup]="trialForm" (ngSubmit)="startTrial()">
            <div class="grid gap-4 md:grid-cols-2">
              <div class="field">
                <label for="trial-start">{{ 'Début de l’essai' | auto }} *</label>
                <input id="trial-start" type="datetime-local" step="1" formControlName="period_started_at" />
              </div>
              <div class="field">
                <label for="trial-end">{{ 'Fin de l’essai' | auto }} *</label>
                <input id="trial-end" type="datetime-local" step="1" formControlName="period_expires_at" />
              </div>
            </div>
            <div class="flex justify-end">
              <button type="submit" class="btn-primary" [disabled]="trialForm.invalid || saving()">
                {{ (saving() ? 'Activation...' : 'Activer l’essai') | auto }}
              </button>
            </div>
          </form>
        }

        <section class="mt-6 border-t border-[var(--line-subtle)] pt-5">
          <div class="flex items-center justify-between gap-3">
            <h4 class="font-black">{{ 'Historique des paiements' | auto }}</h4>
            <span class="text-xs text-[var(--text-muted)]">{{ historyTotal() }} {{ 'paiement(s)' | auto }}</span>
          </div>
          @if (loadingHistory()) {
            <p class="mt-3 text-sm text-[var(--text-muted)]">{{ 'Chargement...' | auto }}</p>
          } @else if (payments().length) {
            <div class="mt-3 overflow-x-auto">
              <table class="data-table w-full border-collapse text-left text-sm">
                <thead>
                  <tr>
                    <th>{{ 'Référence' | auto }}</th>
                    <th>{{ 'Montant' | auto }}</th>
                    <th>{{ 'Paiement' | auto }}</th>
                    <th>{{ 'Période' | auto }}</th>
                    <th>{{ 'Confirmation' | auto }}</th>
                    <th>{{ 'Confirmé par' | auto }}</th>
                  </tr>
                </thead>
                <tbody>
                  @for (payment of payments(); track payment.id) {
                    <tr>
                      <td>{{ payment.payment_reference }}</td>
                      <td>{{ money(payment.amount_fcfa) }}</td>
                      <td>{{ date(payment.paid_at) }}</td>
                      <td>{{ date(payment.period_started_at) }} → {{ date(payment.period_expires_at) }}</td>
                      <td>{{ date(payment.confirmed_at) }}</td>
                      <td>{{ payment.confirmed_by_full_name || payment.confirmed_by_email || payment.confirmed_by_user_id }}</td>
                    </tr>
                  }
                </tbody>
              </table>
            </div>
            @if (historyTotal() > historyPageSize) {
              <div class="mt-3 flex items-center justify-end gap-2">
                <button
                  type="button"
                  class="btn-ghost"
                  [disabled]="historyPage() === 1"
                  (click)="changeHistoryPage(historyPage() - 1)"
                >
                  {{ 'Précédent' | auto }}
                </button>
                <span class="text-xs text-[var(--text-muted)]">{{ 'Page' | auto }} {{ historyPage() }}</span>
                <button
                  type="button"
                  class="btn-ghost"
                  [disabled]="historyPage() * historyPageSize >= historyTotal()"
                  (click)="changeHistoryPage(historyPage() + 1)"
                >
                  {{ 'Suivant' | auto }}
                </button>
              </div>
            }
          } @else {
            <p class="mt-3 text-sm text-[var(--text-muted)]">{{ 'Aucun paiement confirmé.' | auto }}</p>
          }
        </section>
      </div>
    </div>
  `,
})
export class CommuneSubscriptionDialog implements OnInit {
  @Input({ required: true }) communeId = '';
  @Input({ required: true }) communeName = '';
  @Input() subscriptionActive = false;
  @Input() subscriptionEntitlementCurrent = false;
  @Input() subscriptionStartedAt: string | null = null;
  @Input() subscriptionExpiresAt: string | null = null;
  @Output() readonly closed = new EventEmitter<void>();
  @Output() readonly updated = new EventEmitter<void>();

  private readonly api = inject(ApiService);
  private readonly fb = inject(FormBuilder);
  private readonly i18n = inject(I18nService);

  protected readonly mode = signal<'payment' | 'trial'>('payment');
  protected readonly saving = signal(false);
  protected readonly loadingHistory = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly message = signal<string | null>(null);
  protected readonly payments = signal<SubscriptionPayment[]>([]);
  protected readonly historyPage = signal(1);
  protected readonly historyTotal = signal(0);
  protected readonly historyPageSize = 20;

  private readonly nowInput = toLocalDateTime(new Date());
  protected readonly paymentForm = this.fb.nonNullable.group({
    payment_reference: ['', Validators.required],
    amount_fcfa: [null as number | null, [Validators.required, Validators.min(1)]],
    paid_at: [this.nowInput, Validators.required],
    period_started_at: [this.nowInput, Validators.required],
    period_expires_at: ['', Validators.required],
  });
  protected readonly trialForm = this.fb.nonNullable.group({
    period_started_at: [this.nowInput, Validators.required],
    period_expires_at: ['', Validators.required],
  });

  ngOnInit(): void {
    this.subscriptionEntitlementCurrent ||= this.subscriptionActive;
    if (this.subscriptionEntitlementCurrent && this.subscriptionExpiresAt) {
      this.paymentForm.controls.period_started_at.setValue(
        toLocalDateTime(new Date(this.subscriptionExpiresAt)),
      );
    }
    this.loadHistory();
  }

  protected confirmPayment(): void {
    if (this.paymentForm.invalid) {
      this.paymentForm.markAllAsTouched();
      return;
    }
    const raw = this.paymentForm.getRawValue();
    this.saving.set(true);
    this.error.set(null);
    this.message.set(null);
    this.api
      .post(`/api/v1/communes/${this.communeId}/subscription-payments`, {
        payment_reference: raw.payment_reference.trim(),
        amount_fcfa: Number(raw.amount_fcfa),
        paid_at: toIso(raw.paid_at),
        period_started_at: toIso(raw.period_started_at),
        period_expires_at: toIso(raw.period_expires_at),
      })
      .subscribe({
        next: () => {
          this.saving.set(false);
          this.message.set('Paiement confirmé et abonnement mis à jour.');
          this.paymentForm.controls.payment_reference.reset('');
          this.paymentForm.controls.amount_fcfa.reset(null);
          const existingPeriodActive = isCurrentPeriod(
            this.subscriptionStartedAt,
            this.subscriptionExpiresAt,
          );
          const confirmedPeriodActive = isCurrentPeriod(
            raw.period_started_at,
            raw.period_expires_at,
          );
          if (!this.subscriptionEntitlementCurrent || !this.subscriptionStartedAt) {
            this.subscriptionStartedAt = toIso(raw.period_started_at);
          }
          // Un droit futur est bien réservé et doit empêcher un second essai ou
          // un renouvellement discontinu, sans être présenté comme un accès actif.
          this.subscriptionEntitlementCurrent = true;
          this.subscriptionActive = existingPeriodActive || confirmedPeriodActive;
          this.subscriptionExpiresAt = toIso(raw.period_expires_at);
          this.paymentForm.controls.period_started_at.setValue(raw.period_expires_at);
          this.historyPage.set(1);
          this.loadHistory();
          this.updated.emit();
        },
        error: (error: unknown) => {
          this.saving.set(false);
          this.error.set(describeHttpError(error, 'Confirmation du paiement'));
        },
      });
  }

  protected startTrial(): void {
    if (this.trialForm.invalid) {
      this.trialForm.markAllAsTouched();
      return;
    }
    const raw = this.trialForm.getRawValue();
    this.saving.set(true);
    this.error.set(null);
    this.message.set(null);
    this.api
      .post(`/api/v1/communes/${this.communeId}/trial`, {
        period_started_at: toIso(raw.period_started_at),
        period_expires_at: toIso(raw.period_expires_at),
      })
      .subscribe({
        next: () => {
          this.saving.set(false);
          this.message.set('Période d’essai activée.');
          this.subscriptionEntitlementCurrent = true;
          this.subscriptionActive = isCurrentPeriod(raw.period_started_at, raw.period_expires_at);
          this.subscriptionStartedAt = toIso(raw.period_started_at);
          this.subscriptionExpiresAt = toIso(raw.period_expires_at);
          this.paymentForm.controls.period_started_at.setValue(raw.period_expires_at);
          this.updated.emit();
        },
        error: (error: unknown) => {
          this.saving.set(false);
          this.error.set(describeHttpError(error, 'Activation de l’essai'));
        },
      });
  }

  protected date(value: string): string {
    return this.i18n.formatDate(value);
  }

  protected money(value: number): string {
    return this.i18n.formatMoneyFcfa(value);
  }

  protected changeHistoryPage(page: number): void {
    if (page < 1 || (page - 1) * this.historyPageSize >= this.historyTotal()) {
      return;
    }
    this.historyPage.set(page);
    this.loadHistory();
  }

  private loadHistory(): void {
    this.loadingHistory.set(true);
    this.api
      .page<SubscriptionPayment>(`/api/v1/communes/${this.communeId}/subscription-payments`, {
        page: this.historyPage(),
        page_size: this.historyPageSize,
      })
      .subscribe({
        next: (response: Paginated<SubscriptionPayment>) => {
          this.payments.set(response.items);
          this.historyTotal.set(response.total);
          this.loadingHistory.set(false);
        },
        error: (error: unknown) => {
          this.loadingHistory.set(false);
          this.error.set(describeHttpError(error, 'Chargement de l’historique'));
        },
      });
  }
}

function toIso(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toISOString();
}

function toLocalDateTime(date: Date): string {
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 19);
}

function isCurrentPeriod(startValue: string | null, endValue: string | null): boolean {
  if (!startValue || !endValue) {
    return false;
  }
  const start = new Date(startValue).getTime();
  const end = new Date(endValue).getTime();
  const now = Date.now();
  return Number.isFinite(start) && Number.isFinite(end) && start <= now && end >= now;
}
