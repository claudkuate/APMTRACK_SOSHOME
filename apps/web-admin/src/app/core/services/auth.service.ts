import { HttpClient } from '@angular/common/http';
import { Injectable, computed, inject, signal } from '@angular/core';
import { Observable, finalize, shareReplay, tap } from 'rxjs';

import { apiBaseUrl } from '../config/runtime-config';
import { CurrentUser, RoleCode, TokenResponse } from '../../shared/api-types';

@Injectable({ providedIn: 'root' })
export class AuthService {
  private readonly http = inject(HttpClient);
  private readonly baseUrl = apiBaseUrl();
  private readonly tokenState = signal<string | null>(null);
  private readonly userState = signal<CurrentUser | null>(null);
  private readonly accessNoticeState = signal<string | null>(null);
  private restoreAttempted = false;
  private refreshInFlight$: Observable<TokenResponse> | null = null;

  readonly user = this.userState.asReadonly();
  readonly accessToken = this.tokenState.asReadonly();
  readonly accessNotice = this.accessNoticeState.asReadonly();
  readonly isAuthenticated = computed(() => Boolean(this.tokenState() && this.userState()));

  login(email: string, password: string) {
    return this.http
      .post<TokenResponse>(`${this.baseUrl}/api/v1/auth/login`, { email, password })
      .pipe(tap((response) => this.applySession(response)));
  }

  /**
   * Rafraîchit la session via le cookie HttpOnly. Single-flight : l'API fait
   * tourner le refresh token à chaque usage, donc des refresh concurrents
   * (plusieurs 401 simultanés au réveil d'un onglet) s'invalideraient entre
   * eux — tous les appelants partagent ici la même requête en cours.
   */
  refreshCookie(): Observable<TokenResponse> {
    if (!this.refreshInFlight$) {
      this.refreshInFlight$ = this.http
        .post<TokenResponse>(`${this.baseUrl}/api/v1/auth/refresh-cookie`, {})
        .pipe(
          tap((response) => this.applySession(response)),
          finalize(() => (this.refreshInFlight$ = null)),
          shareReplay({ bufferSize: 1, refCount: false }),
        );
    }
    return this.refreshInFlight$;
  }

  me() {
    return this.http
      .get<CurrentUser>(`${this.baseUrl}/api/v1/auth/me`)
      .pipe(tap((user) => this.userState.set(user)));
  }

  restore() {
    if (this.isAuthenticated() || this.restoreAttempted) {
      return this.isAuthenticated();
    }
    this.restoreAttempted = true;
    return null;
  }

  logout() {
    return this.http.post<void>(`${this.baseUrl}/api/v1/auth/logout`, {}).pipe(
      tap({
        next: () => this.clearSession(),
        error: () => this.clearSession(),
      }),
    );
  }

  clearSession(): void {
    this.tokenState.set(null);
    this.userState.set(null);
  }

  blockCommuneAccess(message: string): void {
    this.clearSession();
    this.accessNoticeState.set(message);
  }

  hasAnyRole(roles: RoleCode[]): boolean {
    const current = this.userState();
    if (!current) {
      return false;
    }
    return roles.some((role) => current.roles.includes(role));
  }

  private applySession(response: TokenResponse): void {
    this.accessNoticeState.set(null);
    this.tokenState.set(response.access_token);
    this.userState.set(response.user);
  }
}
