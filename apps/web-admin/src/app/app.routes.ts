import { Routes } from '@angular/router';

import { authGuard } from './core/guards/auth.guard';
import { publicOnlyGuard } from './core/guards/public-only.guard';

export const routes: Routes = [
  {
    path: '',
    pathMatch: 'full',
    redirectTo: 'public/agent',
  },
  {
    path: 'login',
    canActivate: [publicOnlyGuard],
    loadComponent: () =>
      import('./features/auth/login.page').then((module) => module.LoginPage),
  },
  {
    path: 'public',
    loadComponent: () =>
      import('./features/public/public-shell.page').then((module) => module.PublicShellPage),
    children: [
      {
        path: '',
        pathMatch: 'full',
        redirectTo: 'agent',
      },
      {
        path: 'agent',
        loadComponent: () =>
          import('./features/public/public-verify.page').then((module) => module.PublicVerifyPage),
        data: { mode: 'agent' },
      },
      {
        path: 'pv',
        loadComponent: () =>
          import('./features/public/public-verify.page').then((module) => module.PublicVerifyPage),
        data: { mode: 'pv' },
      },
      {
        path: 'signalement',
        loadComponent: () =>
          import('./features/public/public-signalement.page').then(
            (module) => module.PublicSignalementPage,
          ),
      },
      {
        path: 'signalement-suivi',
        loadComponent: () =>
          import('./features/public/public-signalement-tracking.page').then(
            (module) => module.PublicSignalementTrackingPage,
          ),
      },
    ],
  },
  {
    path: '',
    canActivate: [authGuard],
    loadComponent: () =>
      import('./layout/app-shell.component').then((module) => module.AppShellComponent),
    children: [
      {
        path: 'dashboard',
        loadComponent: () =>
          import('./features/dashboard/dashboard.page').then((module) => module.DashboardPage),
      },
      {
        path: 'status',
        loadComponent: () =>
          import('./features/status/status').then((module) => module.Status),
      },
      {
        path: 'carte',
        loadComponent: () =>
          import('./features/carte/carte-map.page').then((module) => module.CarteMapPage),
      },
      {
        path: 'payments',
        loadComponent: () =>
          import('./features/payments/payments.page').then((module) => module.PaymentsPage),
      },
      {
        path: 'settings',
        loadComponent: () =>
          import('./features/settings/settings.page').then((module) => module.SettingsPage),
      },
      {
        path: 'search',
        loadComponent: () =>
          import('./features/search/search.page').then((module) => module.SearchPage),
      },
      {
        path: 'exports',
        loadComponent: () =>
          import('./features/exports/exports.page').then((module) => module.ExportsPage),
      },
      {
        path: ':feature/:id',
        loadComponent: () =>
          import('./features/resource-detail/resource-detail.page').then((module) => module.ResourceDetailPage),
      },
      {
        path: ':feature',
        loadComponent: () =>
          import('./features/resource/resource.page').then((module) => module.ResourcePage),
      },
    ],
  },
  {
    path: '**',
    redirectTo: 'dashboard',
  },
];
