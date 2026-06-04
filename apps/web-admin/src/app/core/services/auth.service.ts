import { HttpClient } from '@angular/common/http';
import { Injectable, computed, inject, signal } from '@angular/core';
import { tap } from 'rxjs';

import { apiBaseUrl } from '../config/runtime-config';
import { CurrentUser, RoleCode, TokenResponse } from '../../shared/api-types';

@Injectable({ providedIn: 'root' })
export class AuthService {
  private readonly http = inject(HttpClient);
  private readonly baseUrl = apiBaseUrl();
  private readonly tokenState = signal<string | null>(null);
  private readonly userState = signal<CurrentUser | null>(null);
  private restoreAttempted = false;

  readonly user = this.userState.asReadonly();
  readonly accessToken = this.tokenState.asReadonly();
  readonly isAuthenticated = computed(() => Boolean(this.tokenState() && this.userState()));

  login(email: string, password: string) {
    return this.http
      .post<TokenResponse>(`${this.baseUrl}/api/v1/auth/login`, { email, password })
      .pipe(tap((response) => this.applySession(response)));
  }

  refreshCookie() {
    return this.http
      .post<TokenResponse>(`${this.baseUrl}/api/v1/auth/refresh-cookie`, {})
      .pipe(tap((response) => this.applySession(response)));
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

  hasAnyRole(roles: RoleCode[]): boolean {
    const current = this.userState();
    if (!current) {
      return false;
    }
    return roles.some((role) => current.roles.includes(role));
  }

  private applySession(response: TokenResponse): void {
    this.tokenState.set(response.access_token);
    this.userState.set(response.user);
  }
}
