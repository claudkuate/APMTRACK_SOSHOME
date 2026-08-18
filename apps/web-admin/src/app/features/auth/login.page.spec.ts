import { provideHttpClient } from '@angular/common/http';
import { HttpTestingController, provideHttpClientTesting } from '@angular/common/http/testing';
import { ComponentFixture, TestBed } from '@angular/core/testing';
import { provideRouter } from '@angular/router';

import { LoginPage } from './login.page';

describe('LoginPage - abonnement mairie', () => {
  let fixture: ComponentFixture<LoginPage>;
  let http: HttpTestingController;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [LoginPage],
      providers: [provideHttpClient(), provideHttpClientTesting(), provideRouter([])],
    }).compileComponents();

    fixture = TestBed.createComponent(LoginPage);
    fixture.detectChanges();
    http = TestBed.inject(HttpTestingController);
  });

  afterEach(() => http.verify());

  it('affiche sans le remplacer le message métier du 403 abonnement', () => {
    const message =
      "L’accès de votre mairie est suspendu ou son abonnement n’est pas valide. Contactez l’administrateur de la plateforme.";
    setInput('#email', 'agent@test.local');
    setInput('#password', 'mot-de-passe-valide');

    const form = fixture.nativeElement.querySelector('form') as HTMLFormElement;
    form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));

    http
      .expectOne((request) => request.url.endsWith('/api/v1/auth/login'))
      .flush(
        { error: { code: 'COMMUNE_SUBSCRIPTION_INACTIVE', message } },
        { status: 403, statusText: 'Forbidden' },
      );
    fixture.detectChanges();

    expect(fixture.nativeElement.textContent).toContain(message);
    expect(fixture.nativeElement.textContent).not.toContain(
      'Identifiants invalides ou compte inactif.',
    );
  });

  function setInput(selector: string, value: string): void {
    const input = fixture.nativeElement.querySelector(selector) as HTMLInputElement;
    input.value = value;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    fixture.detectChanges();
  }
});
