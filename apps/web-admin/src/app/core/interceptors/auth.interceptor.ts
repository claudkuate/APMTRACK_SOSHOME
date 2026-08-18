import { HttpErrorResponse, HttpInterceptorFn } from '@angular/common/http';
import { inject } from '@angular/core';
import { Router } from '@angular/router';
import { catchError, switchMap, throwError } from 'rxjs';

import { apiBaseUrl } from '../config/runtime-config';
import { AuthService } from '../services/auth.service';

export const authInterceptor: HttpInterceptorFn = (request, next) => {
  // Ne jamais joindre le jeton ni les cookies aux requêtes externes (ex. Nominatim/OSM).
  const isApiRequest =
    request.url.startsWith(apiBaseUrl()) || request.url.startsWith('/api/');
  if (!isApiRequest) {
    return next(request);
  }

  const auth = inject(AuthService);
  const router = inject(Router);
  const token = auth.accessToken();
  const isAuthRoute = request.url.includes('/api/v1/auth/');
  const authenticatedRequest = request.clone({
    withCredentials: true,
    setHeaders: token ? { Authorization: `Bearer ${token}` } : {},
  });

  return next(authenticatedRequest).pipe(
    catchError((error: unknown) => {
      if (error instanceof HttpErrorResponse && isCommuneSubscriptionError(error)) {
        const message = readApiMessage(error);
        auth.blockCommuneAccess(message);
        void router.navigateByUrl('/login');
        return throwError(() => error);
      }
      if (
        error instanceof HttpErrorResponse &&
        error.status === 401 &&
        !isAuthRoute &&
        !request.url.includes('/api/v1/public/')
      ) {
        return auth.refreshCookie().pipe(
          switchMap(() => {
            const refreshedToken = auth.accessToken();
            return next(
              request.clone({
                withCredentials: true,
                setHeaders: refreshedToken ? { Authorization: `Bearer ${refreshedToken}` } : {},
              }),
            );
          }),
          catchError((refreshError) => {
            // Ce catch englobe aussi l'erreur éventuelle de la requête rejouée.
            // Si le refresh a réussi puis que le replay révèle une suspension,
            // conserver le message métier et éviter tout nouveau cycle de refresh.
            if (
              refreshError instanceof HttpErrorResponse &&
              isCommuneSubscriptionError(refreshError)
            ) {
              auth.blockCommuneAccess(readApiMessage(refreshError));
              void router.navigateByUrl('/login');
            } else {
              auth.clearSession();
            }
            return throwError(() => refreshError);
          }),
        );
      }

      return throwError(() => error);
    }),
  );
};

function isCommuneSubscriptionError(error: HttpErrorResponse): boolean {
  const body = error.error as { error?: { code?: unknown } } | null | undefined;
  return error.status === 403 && body?.error?.code === 'COMMUNE_SUBSCRIPTION_INACTIVE';
}

function readApiMessage(error: HttpErrorResponse): string {
  const body = error.error as { error?: { message?: unknown } } | null | undefined;
  const message = body?.error?.message;
  return typeof message === 'string' && message.trim()
    ? message
    : "L’accès de votre mairie est suspendu ou son abonnement n’est pas valide. Contactez l’administrateur de la plateforme.";
}
