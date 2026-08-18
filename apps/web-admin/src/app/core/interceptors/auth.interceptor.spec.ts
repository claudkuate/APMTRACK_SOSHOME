import { HttpClient, provideHttpClient, withInterceptors } from '@angular/common/http';
import { HttpTestingController, provideHttpClientTesting } from '@angular/common/http/testing';
import { TestBed } from '@angular/core/testing';
import { Router } from '@angular/router';

import { TokenResponse } from '../../shared/api-types';
import { AuthService } from '../services/auth.service';
import { authInterceptor } from './auth.interceptor';

class RouterStub {
  navigatedTo: string | null = null;

  navigateByUrl(url: string): Promise<boolean> {
    this.navigatedTo = url;
    return Promise.resolve(true);
  }
}

const session: TokenResponse = {
  access_token: 'subscription-test-token',
  token_type: 'Bearer',
  expires_in_seconds: 900,
  user: {
    id: 'commune-user',
    email: 'agent@example.test',
    full_name: 'Agent Test',
    commune_id: 'commune-1',
    roles: ['APM_AGENT'],
    active: true,
  },
};

describe('authInterceptor - abonnement mairie', () => {
  let client: HttpClient;
  let http: HttpTestingController;
  let auth: AuthService;
  let router: RouterStub;

  beforeEach(() => {
    TestBed.configureTestingModule({
      providers: [
        provideHttpClient(withInterceptors([authInterceptor])),
        provideHttpClientTesting(),
        { provide: Router, useClass: RouterStub },
      ],
    });
    client = TestBed.inject(HttpClient);
    http = TestBed.inject(HttpTestingController);
    auth = TestBed.inject(AuthService);
    router = TestBed.inject(Router) as unknown as RouterStub;

    auth.refreshCookie().subscribe();
    http
      .expectOne((req) => req.url.endsWith('/api/v1/auth/refresh-cookie'))
      .flush(session);
  });

  afterEach(() => http.verify());

  it('ne tente pas de refresh sur le 403 dedie et redirige sans boucle', () => {
    const message =
      "L’accès de votre mairie est suspendu ou son abonnement n’est pas valide. Contactez l’administrateur de la plateforme.";
    let failed = false;

    client.get('/api/v1/pvs').subscribe({ error: () => (failed = true) });
    const request = http.expectOne('/api/v1/pvs');
    expect(request.request.headers.get('Authorization')).toBe('Bearer subscription-test-token');
    request.flush(
      { error: { code: 'COMMUNE_SUBSCRIPTION_INACTIVE', message } },
      { status: 403, statusText: 'Forbidden' },
    );

    expect(failed).toBe(true);
    expect(auth.isAuthenticated()).toBe(false);
    expect(auth.accessNotice()).toBe(message);
    expect(router.navigatedTo).toBe('/login');
    expect(http.match((req) => req.url.includes('/auth/refresh')).length).toBe(0);
  });

  it('conserve le message si le 403 dedie arrive sur la requete rejouee apres refresh', () => {
    const message =
      "L’accès de votre mairie est suspendu ou son abonnement n’est pas valide. Contactez l’administrateur de la plateforme.";
    let failed = false;

    client.get('/api/v1/pvs').subscribe({ error: () => (failed = true) });
    http
      .expectOne('/api/v1/pvs')
      .flush(
        { error: { code: 'UNAUTHORIZED', message: 'Jeton expiré' } },
        { status: 401, statusText: 'Unauthorized' },
      );

    http
      .expectOne((req) => req.url.endsWith('/api/v1/auth/refresh-cookie'))
      .flush({ ...session, access_token: 'refreshed-subscription-token' });

    const replay = http.expectOne('/api/v1/pvs');
    expect(replay.request.headers.get('Authorization')).toBe(
      'Bearer refreshed-subscription-token',
    );
    replay.flush(
      { error: { code: 'COMMUNE_SUBSCRIPTION_INACTIVE', message } },
      { status: 403, statusText: 'Forbidden' },
    );

    expect(failed).toBe(true);
    expect(auth.isAuthenticated()).toBe(false);
    expect(auth.accessNotice()).toBe(message);
    expect(router.navigatedTo).toBe('/login');
    expect(http.match((req) => req.url.includes('/auth/refresh')).length).toBe(0);
  });
});
