import { Component, HostListener, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { Router, RouterLink } from '@angular/router';

import { I18nService } from '../../core/i18n/i18n.service';
import { AutoTranslatePipe } from '../../core/i18n/auto-translate.pipe';
import { AuthService } from '../../core/services/auth.service';

@Component({
  selector: 'app-login-page',
  imports: [ReactiveFormsModule, RouterLink, AutoTranslatePipe],
  template: `
    <main class="relative min-h-screen w-full overflow-hidden bg-[var(--sidebar-bottom)]">
      <!-- Couche image plein écran (parallaxe souris) -->
      <img
        src="/yaounde-reunification-login-hero.png"
        alt="Vue futuriste sobre du monument de la Reunification a Yaounde"
        class="pointer-events-none absolute -inset-[6%] h-[112%] w-[112%] select-none object-cover [transition:transform_140ms_ease-out] [will-change:transform]"
        [style.transform]="imageTransform()"
      />

      <!-- Overlay dégradé pour la lisibilité du texte clair -->
      <div
        class="pointer-events-none absolute inset-0 z-10 bg-gradient-to-r from-[var(--sidebar-bottom)]/92 via-[var(--sidebar-top)]/70 to-black/35"
      ></div>
      <div
        class="pointer-events-none absolute inset-0 z-10 bg-gradient-to-t from-black/50 via-transparent to-black/20"
      ></div>

      <!-- Contenu -->
      <div class="relative z-20 flex min-h-screen flex-col">
        <!-- Brand -->
        <header class="flex items-center gap-3 p-5 md:p-7">
          <span class="side-emblem h-12 w-12">
            <img class="brand-logo" src="/armoiries-cameroun.svg" alt="Armoiries de la République du Cameroun" />
          </span>
          <div class="min-w-0 text-white">
            <strong class="block text-xl">G-APM</strong>
            <span class="text-sm text-slate-200">{{ 'Gestion des Activités de Police Municipale' | auto }}</span>
          </div>
        </header>

        <!-- Corps : texte + carte -->
        <div
          class="flex flex-1 flex-col items-stretch gap-8 px-5 pb-6 md:px-7 lg:flex-row lg:items-center lg:justify-between lg:gap-10"
        >
          <div
            class="min-w-0 max-w-xl text-white [transition:transform_180ms_ease-out] [will-change:transform]"
            [style.transform]="textTransform()"
          >
            <p class="mb-3 text-sm font-bold uppercase tracking-wide text-[var(--cameroon-yellow)]">{{ 'Back-office' | auto }}</p>
            <h1 class="break-words text-3xl font-black leading-tight drop-shadow-[0_2px_12px_rgba(0,0,0,0.45)] sm:text-4xl md:text-5xl">
              {{ 'Administration communale sobre et tracable.' | auto }}
            </h1>
            <p class="mt-5 max-w-lg text-slate-100 drop-shadow-[0_1px_8px_rgba(0,0,0,0.5)]">
              {{ 'Acces reserve aux profils autorises. Les roles et les restrictions de commune sont verifies par le backend.' | auto }}
            </p>

            <nav class="mt-7 flex flex-wrap gap-2 text-sm">
              <a routerLink="/public/agent" class="btn-ghost border-white/25 bg-white/10 text-white backdrop-blur-sm">
                {{ 'Verifier agent' | auto }}
              </a>
              <a routerLink="/public/pv" class="btn-ghost border-white/25 bg-white/10 text-white backdrop-blur-sm">
                {{ 'Verifier PV' | auto }}
              </a>
              <a
                routerLink="/public/signalement"
                class="btn-ghost border-white/25 bg-white/10 text-white backdrop-blur-sm"
              >
                {{ 'Signalement' | auto }}
              </a>
            </nav>
          </div>

          <form
            class="panel w-full min-w-0 shrink-0 p-5 shadow-[var(--shadow-pop)] sm:p-6 lg:max-w-md"
            [formGroup]="form"
            (ngSubmit)="submit()"
          >
            <p class="text-xs font-bold uppercase text-[var(--text-muted)]">{{ 'Session' | auto }}</p>
            <h2 class="mt-1 text-2xl font-black">{{ 'Connexion' | auto }}</h2>
            <p class="mt-2 text-sm text-[var(--text-muted)]">{{ 'Utilise un compte G-APM actif.' | auto }}</p>

            <div class="mt-6 grid gap-4">
              <div class="field">
                <label for="email">Email</label>
                <input id="email" type="email" formControlName="email" autocomplete="email" />
              </div>
              <div class="field">
                <label for="password">{{ 'Mot de passe' | auto }}</label>
                <input id="password" type="password" formControlName="password" autocomplete="current-password" />
              </div>
            </div>

            @if (error()) {
              <p class="mt-4 rounded-md border border-red-200 bg-red-50 p-3 text-sm font-semibold text-red-800">
                {{ error() }}
              </p>
            }

            <button type="submit" class="btn-primary mt-6 w-full" [disabled]="form.invalid || loading()">
              {{ (loading() ? 'Connexion...' : 'Se connecter') | auto }}
            </button>
          </form>
        </div>
      </div>
    </main>
  `,
})
export class LoginPage {
  private readonly fb = inject(FormBuilder);
  private readonly auth = inject(AuthService);
  private readonly router = inject(Router);
  private readonly i18n = inject(I18nService);

  protected readonly loading = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly form = this.fb.nonNullable.group({
    email: ['', [Validators.required, Validators.email]],
    password: ['', [Validators.required, Validators.minLength(8)]],
  });

  // Décalage parallaxe normalisé dans [-1, 1] depuis le centre de la fenêtre.
  private readonly offset = signal({ x: 0, y: 0 });

  protected imageTransform(): string {
    const { x, y } = this.offset();
    // Amplitude la plus marquée pour la couche de fond (effet de profondeur).
    return `translate3d(${x * 18}px, ${y * 18}px, 0) scale(1.06)`;
  }

  protected textTransform(): string {
    const { x, y } = this.offset();
    // Amplitude plus faible et inversée pour la couche de premier plan.
    return `translate3d(${x * -7}px, ${y * -7}px, 0)`;
  }

  @HostListener('window:mousemove', ['$event'])
  protected onMouseMove(event: MouseEvent): void {
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
      return;
    }
    const x = (event.clientX / window.innerWidth) * 2 - 1;
    const y = (event.clientY / window.innerHeight) * 2 - 1;
    this.offset.set({ x, y });
  }

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
        this.error.set(this.i18n.auto('Identifiants invalides ou compte inactif.'));
        this.loading.set(false);
      },
    });
  }
}
