import { Component, signal } from '@angular/core';
import { RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';

@Component({
  selector: 'app-public-shell-page',
  imports: [RouterLink, RouterLinkActive, RouterOutlet],
  template: `
    <main class="relative min-h-screen">
      <!-- Fond : photo de Yaoundé fixe + voile clair pour la lisibilité -->
      <img
        src="/yaounde-login-hero.png"
        alt=""
        aria-hidden="true"
        class="pointer-events-none fixed inset-0 -z-20 h-full w-full select-none object-cover"
      />
      <div class="pointer-events-none fixed inset-0 -z-10 bg-[var(--surface-canvas)]/88 backdrop-blur-[2px]"></div>

      <header class="border-b border-[var(--line-subtle)] bg-white">
        <!-- Bande drapeau camerounais : vert | rouge | jaune -->
        <div class="flex h-1.5 w-full" aria-hidden="true">
          <span class="flex-1 bg-[var(--cameroon-green)]"></span>
          <span class="flex-1 bg-[var(--cameroon-red)]"></span>
          <span class="flex-1 bg-[var(--cameroon-gold)]"></span>
        </div>
        <div class="mx-auto flex max-w-6xl flex-wrap items-center justify-between gap-3 px-4 py-4">
          <a routerLink="/public/agent" class="grid min-w-0 grid-cols-[40px_1fr] items-center gap-3">
            <span class="side-emblem h-10 w-10">
              <img class="brand-logo" src="/armoiries-cameroun.svg" alt="Armoiries de la République du Cameroun" />
            </span>
            <span class="min-w-0">
              <strong class="block leading-tight">APMTRACK</strong>
              <small class="text-[var(--text-muted)]">Portail public</small>
            </span>
          </a>
          <nav class="flex flex-wrap items-center gap-2 text-sm">
            <a routerLink="/public/agent" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">
              {{ label('agent') }}
            </a>
            <a routerLink="/public/pv" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">PV</a>
            <a routerLink="/public/signalement" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">
              {{ label('report') }}
            </a>
            <a routerLink="/public/signalement-suivi" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">
              {{ label('tracking') }}
            </a>
            <a routerLink="/public/missions" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">
              {{ label('missions') }}
            </a>
            <select
              class="rounded-md border border-[var(--line-subtle)] bg-white px-2 py-2 font-semibold"
              [value]="language()"
              (change)="setLanguage($any($event.target).value)"
              aria-label="Langue"
            >
              <option value="fr">FR</option>
              <option value="en">EN</option>
            </select>
          </nav>
        </div>
      </header>

      <section class="relative mx-auto max-w-6xl px-4 py-7">
        <router-outlet />
      </section>
    </main>
  `,
})
export class PublicShellPage {
  protected readonly language = signal(localStorage.getItem('apmtrack.lang') === 'en' ? 'en' : 'fr');

  protected setLanguage(value: string): void {
    const language = value === 'en' ? 'en' : 'fr';
    localStorage.setItem('apmtrack.lang', language);
    this.language.set(language);
  }

  protected label(key: 'agent' | 'report' | 'tracking' | 'missions'): string {
    const fr = {
      agent: 'Agent',
      report: 'Signaler',
      tracking: 'Suivi',
      missions: 'Missions',
    };
    const en = {
      agent: 'Officer',
      report: 'Report',
      tracking: 'Tracking',
      missions: 'Missions',
    };
    return (this.language() === 'en' ? en : fr)[key];
  }
}
