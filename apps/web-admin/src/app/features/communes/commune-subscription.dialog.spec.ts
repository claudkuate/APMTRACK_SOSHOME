import { provideHttpClient } from '@angular/common/http';
import { HttpTestingController, provideHttpClientTesting } from '@angular/common/http/testing';
import { ComponentFixture, TestBed } from '@angular/core/testing';

import { CommuneSubscriptionDialog } from './commune-subscription.dialog';

describe('CommuneSubscriptionDialog', () => {
  let fixture: ComponentFixture<CommuneSubscriptionDialog>;
  let http: HttpTestingController;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [CommuneSubscriptionDialog],
      providers: [provideHttpClient(), provideHttpClientTesting()],
    }).compileComponents();

    fixture = TestBed.createComponent(CommuneSubscriptionDialog);
    fixture.componentRef.setInput('communeId', 'commune-1');
    fixture.componentRef.setInput('communeName', 'Commune test');
    http = TestBed.inject(HttpTestingController);
  });

  afterEach(() => http.verify());

  it('traite une période courante comme un droit même si la mairie est suspendue', () => {
    const start = new Date(Date.now() - 7 * 24 * 60 * 60_000);
    const expiry = new Date(Date.now() + 7 * 24 * 60 * 60_000);
    fixture.componentRef.setInput('subscriptionActive', false);
    fixture.componentRef.setInput('subscriptionEntitlementCurrent', true);
    fixture.componentRef.setInput('subscriptionStartedAt', start.toISOString());
    fixture.componentRef.setInput('subscriptionExpiresAt', expiry.toISOString());
    fixture.detectChanges();
    flushEmptyHistory();
    fixture.detectChanges();

    const trialButton = buttonContaining('Ouvrir une période d’essai');
    const renewalStart = fixture.nativeElement.querySelector(
      '#subscription-start',
    ) as HTMLInputElement;

    expect(trialButton.disabled).toBe(true);
    expect(renewalStart.readOnly).toBe(true);
    expect(renewalStart.value.replace('.000', '')).toBe(toLocalInput(expiry));
  });

  it('ne marque pas actif un paiement dont la période commence dans le futur', () => {
    fixture.componentRef.setInput('subscriptionActive', false);
    fixture.componentRef.setInput('subscriptionEntitlementCurrent', false);
    fixture.detectChanges();
    flushEmptyHistory();

    const start = futureDate(1);
    const end = futureDate(31);
    setInput('#subscription-reference', 'PAY-FUTURE-1');
    setInput('#subscription-amount', '25000');
    setInput('#subscription-start', toLocalInput(start));
    setInput('#subscription-end', toLocalInput(end));
    submit('form');

    const request = http.expectOne((req) =>
      req.url.endsWith('/api/v1/communes/commune-1/subscription-payments'),
    );
    expect(request.request.body.period_started_at).toBe(start.toISOString());
    request.flush({});
    flushEmptyHistory();
    fixture.detectChanges();

    expect(fixture.componentInstance.subscriptionActive).toBe(false);
    expect(fixture.componentInstance.subscriptionEntitlementCurrent).toBe(true);
    expect(
      (fixture.nativeElement.querySelector('#subscription-start') as HTMLInputElement).readOnly,
    ).toBe(true);
    expect(buttonContaining('Ouvrir une période d’essai').disabled).toBe(true);
    expect(fixture.nativeElement.textContent).toContain(
      'Droit d’abonnement confirmé, mais accès actuellement inactif ou non commencé.',
    );
  });

  it('ne marque pas actif un essai dont la période commence dans le futur', () => {
    fixture.componentRef.setInput('subscriptionActive', false);
    fixture.componentRef.setInput('subscriptionEntitlementCurrent', false);
    fixture.detectChanges();
    flushEmptyHistory();

    buttonContaining('Ouvrir une période d’essai').click();
    fixture.detectChanges();
    const start = futureDate(1);
    const end = futureDate(8);
    setInput('#trial-start', toLocalInput(start));
    setInput('#trial-end', toLocalInput(end));
    submit('form');

    const request = http.expectOne((req) =>
      req.url.endsWith('/api/v1/communes/commune-1/trial'),
    );
    expect(request.request.body.period_started_at).toBe(start.toISOString());
    request.flush({});
    fixture.detectChanges();

    expect(fixture.componentInstance.subscriptionActive).toBe(false);
    expect(fixture.componentInstance.subscriptionEntitlementCurrent).toBe(true);
    expect(buttonContaining('Ouvrir une période d’essai').disabled).toBe(true);
    expect(fixture.nativeElement.textContent).toContain(
      'Droit d’abonnement confirmé, mais accès actuellement inactif ou non commencé.',
    );
  });

  it('affiche la référence, la période et le confirmateur de l’historique', () => {
    fixture.detectChanges();
    http
      .expectOne((req) =>
        req.url.endsWith('/api/v1/communes/commune-1/subscription-payments'),
      )
      .flush({
        items: [
          {
            id: 'payment-1',
            payment_reference: 'REF-HIST-001',
            amount_fcfa: 50000,
            paid_at: '2026-08-01T09:00:00Z',
            period_started_at: '2026-08-01T09:00:00Z',
            period_expires_at: '2026-09-01T09:00:00Z',
            confirmed_at: '2026-08-01T09:05:00Z',
            confirmed_by_user_id: 'root-1',
            confirmed_by_full_name: 'Super Administrateur',
          },
        ],
        page: 1,
        page_size: 20,
        total: 1,
      });
    fixture.detectChanges();

    const text = String(fixture.nativeElement.textContent);
    expect(text).toContain('REF-HIST-001');
    expect(text.replace(/\s/g, '')).toContain('50000FCFA');
    expect(text).toContain('Super Administrateur');
  });

  function flushEmptyHistory(): void {
    http
      .expectOne((req) =>
        req.url.endsWith('/api/v1/communes/commune-1/subscription-payments'),
      )
      .flush({ items: [], page: 1, page_size: 20, total: 0 });
  }

  function buttonContaining(label: string): HTMLButtonElement {
    const button = Array.from(
      fixture.nativeElement.querySelectorAll('button') as NodeListOf<HTMLButtonElement>,
    ).find((candidate) => candidate.textContent?.includes(label));
    if (!button) {
      throw new Error(`Bouton introuvable : ${label}`);
    }
    return button;
  }

  function setInput(selector: string, value: string): void {
    const input = fixture.nativeElement.querySelector(selector) as HTMLInputElement | null;
    if (!input) {
      throw new Error(`Champ introuvable : ${selector}`);
    }
    input.value = value;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    fixture.detectChanges();
  }

  function submit(selector: string): void {
    const form = fixture.nativeElement.querySelector(selector) as HTMLFormElement | null;
    if (!form) {
      throw new Error(`Formulaire introuvable : ${selector}`);
    }
    form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    fixture.detectChanges();
  }
});

function toLocalInput(date: Date): string {
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 19);
}

function futureDate(days: number): Date {
  const date = new Date(Date.now() + days * 24 * 60 * 60_000);
  date.setMilliseconds(0);
  return date;
}
