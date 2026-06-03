import { Routes } from '@angular/router';

export const routes: Routes = [
  {
    path: '',
    pathMatch: 'full',
    redirectTo: 'status',
  },
  {
    path: 'status',
    loadComponent: () =>
      import('./features/status/status').then((module) => module.Status),
  },
  {
    path: '**',
    redirectTo: 'status',
  },
];
