import { provideHttpClient } from '@angular/common/http';
import { HttpTestingController, provideHttpClientTesting } from '@angular/common/http/testing';
import { TestBed } from '@angular/core/testing';

import { TokenResponse } from '../../shared/api-types';
import { AuthService } from './auth.service';

const tokenResponse = (accessToken: string): TokenResponse => ({
  access_token: accessToken,
  token_type: 'Bearer',
  expires_in_seconds: 900,
  user: {
    id: 'user-1',
    email: 'admin@test.local',
    full_name: 'Admin Test',
    commune_id: null,
    roles: ['SUPER_ADMIN'],
    active: true,
  },
});

describe('AuthService.refreshCookie', () => {
  let service: AuthService;
  let http: HttpTestingController;

  beforeEach(() => {
    TestBed.configureTestingModule({
      providers: [provideHttpClient(), provideHttpClientTesting()],
    });
    service = TestBed.inject(AuthService);
    http = TestBed.inject(HttpTestingController);
  });

  afterEach(() => http.verify());

  it('partage un unique refresh entre abonnés concurrents (rotation du refresh token)', () => {
    let delivered = 0;
    service.refreshCookie().subscribe(() => delivered++);
    service.refreshCookie().subscribe(() => delivered++);

    // Un seul POST malgré deux appels simultanés : la rotation côté API
    // invaliderait le second refresh s'il partait en parallèle.
    const pending = http.match((req) => req.url.endsWith('/api/v1/auth/refresh-cookie'));
    expect(pending.length).toBe(1);
    pending[0].flush(tokenResponse('shared-access'));

    expect(delivered).toBe(2);
    expect(service.accessToken()).toBe('shared-access');
  });

  it('relance une vraie requête une fois le refresh précédent terminé', () => {
    service.refreshCookie().subscribe();
    http
      .expectOne((req) => req.url.endsWith('/api/v1/auth/refresh-cookie'))
      .flush(tokenResponse('first'));

    service.refreshCookie().subscribe();
    http
      .expectOne((req) => req.url.endsWith('/api/v1/auth/refresh-cookie'))
      .flush(tokenResponse('second'));

    expect(service.accessToken()).toBe('second');
  });

  it("un refresh en échec n'empoisonne pas les suivants", () => {
    let failed = false;
    service.refreshCookie().subscribe({ error: () => (failed = true) });
    http
      .expectOne((req) => req.url.endsWith('/api/v1/auth/refresh-cookie'))
      .flush({ error: { code: 'UNAUTHORIZED', message: 'expiré' } }, { status: 401, statusText: 'Unauthorized' });
    expect(failed).toBe(true);

    service.refreshCookie().subscribe();
    http
      .expectOne((req) => req.url.endsWith('/api/v1/auth/refresh-cookie'))
      .flush(tokenResponse('recovered'));
    expect(service.accessToken()).toBe('recovered');
  });
});
