import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';
import { catchError, map, of } from 'rxjs';

import { AuthService } from '../services/auth.service';

export const authGuard: CanActivateFn = () => {
  const auth = inject(AuthService);
  const router = inject(Router);
  const restored = auth.restore();

  if (restored) {
    return true;
  }

  return auth.refreshCookie().pipe(
    map(() => true),
    catchError(() => of(router.createUrlTree(['/login']))),
  );
};
