import { Component, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { Router, RouterLink } from '@angular/router';

import { AuthService } from '../../core/services/auth.service';

@Component({
  selector: 'app-login-page',
  imports: [ReactiveFormsModule, RouterLink],
  template: `
    <main class="grid min-h-screen grid-cols-1 overflow-x-hidden bg-[var(--surface-canvas)] lg:grid-cols-[0.95fr_1.05fr]">
      <section class="flex min-h-[38vh] min-w-0 flex-col justify-between bg-gradient-to-b from-[var(--sidebar-top)] to-[var(--sidebar-bottom)] p-5 text-white md:p-7 lg:min-h-screen">
        <div class="grid min-w-0 grid-cols-[48px_1fr] items-center gap-3">
          <span class="side-emblem h-12 w-12">
            <img class="brand-logo" src="/armoiries-cameroun.svg" alt="Armoiries de la République du Cameroun" />
          </span>
          <div class="min-w-0">
            <strong class="block text-xl">APMTRACK</strong>
            <span class="text-sm text-slate-300">Gestion Police Municipale</span>
          </div>
        </div>

        <div class="min-w-0 max-w-xl">
          <p class="mb-3 text-sm font-bold uppercase text-[var(--cameroon-yellow)]">Back-office</p>
          <h1 class="break-words text-2xl font-black leading-tight sm:text-3xl md:text-5xl">
            Administration communale sobre et tracable.
          </h1>
          <p class="mt-5 max-w-lg text-slate-300">
            Acces reserve aux profils autorises. Les roles et les restrictions de commune sont verifies par le backend.
          </p>
        </div>

        <nav class="flex flex-wrap gap-2 text-sm">
          <a routerLink="/public/agent" class="btn-ghost border-white/20 bg-white/5 text-white">Verifier agent</a>
          <a routerLink="/public/pv" class="btn-ghost border-white/20 bg-white/5 text-white">Verifier PV</a>
          <a routerLink="/public/signalement" class="btn-ghost border-white/20 bg-white/5 text-white">
            Signalement
          </a>
        </nav>
      </section>

      <section class="flex min-w-0 items-center justify-center p-4 sm:p-5">
        <form class="panel w-full min-w-0 max-w-md p-5 shadow-[var(--shadow-soft)] sm:p-6" [formGroup]="form" (ngSubmit)="submit()">
          <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Session</p>
          <h2 class="mt-1 text-2xl font-black">Connexion</h2>
          <p class="mt-2 text-sm text-[var(--text-muted)]">Utilise un compte APMTRACK actif.</p>

          <div class="mt-6 grid gap-4">
            <div class="field">
              <label for="email">Email</label>
              <input id="email" type="email" formControlName="email" autocomplete="email" />
            </div>
            <div class="field">
              <label for="password">Mot de passe</label>
              <input id="password" type="password" formControlName="password" autocomplete="current-password" />
            </div>
          </div>

          @if (error()) {
            <p class="mt-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm font-semibold text-red-800">
              {{ error() }}
            </p>
          }

          <button type="submit" class="btn-primary mt-6 w-full" [disabled]="form.invalid || loading()">
            {{ loading() ? 'Connexion...' : 'Se connecter' }}
          </button>
        </form>
      </section>
    </main>
  `,
})
export class LoginPage {
  private readonly fb = inject(FormBuilder);
  private readonly auth = inject(AuthService);
  private readonly router = inject(Router);

  protected readonly loading = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly form = this.fb.nonNullable.group({
    email: ['', [Validators.required, Validators.email]],
    password: ['', [Validators.required, Validators.minLength(8)]],
  });

  protected submit(): void {
    if (this.form.invalid) {
      return;
    }

    this.loading.set(true);
    this.error.set(null);
    const { email, password } = this.form.getRawValue();
    this.auth.login(email, password).subscribe({
      next: () => this.router.navigateByUrl('/dashboard'),
      error: () => {
        this.error.set('Identifiants invalides ou compte inactif.');
        this.loading.set(false);
      },
    });
  }
}
