import { HttpErrorResponse, HttpInterceptorFn } from '@angular/common/http';
import { inject } from '@angular/core';
import { catchError, switchMap, throwError } from 'rxjs';

import { AuthService } from '../services/auth.service';

export const authInterceptor: HttpInterceptorFn = (request, next) => {
  const auth = inject(AuthService);
  const token = auth.accessToken();
  const isAuthRoute = request.url.includes('/api/v1/auth/');
  const authenticatedRequest = request.clone({
    withCredentials: true,
    setHeaders: token ? { Authorization: `Bearer ${token}` } : {},
  });

  return next(authenticatedRequest).pipe(
    catchError((error: unknown) => {
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
            auth.clearSession();
            return throwError(() => refreshError);
          }),
        );
      }

      return throwError(() => error);
    }),
  );
};
