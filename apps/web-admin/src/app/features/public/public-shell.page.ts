import { Component } from '@angular/core';
import { RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';

@Component({
  selector: 'app-public-shell-page',
  imports: [RouterLink, RouterLinkActive, RouterOutlet],
  template: `
    <main class="min-h-screen bg-[var(--surface-canvas)]">
      <header class="border-b border-[var(--line-subtle)] bg-white">
        <div class="mx-auto flex max-w-6xl flex-wrap items-center justify-between gap-3 px-4 py-4">
          <a routerLink="/public/agent" class="grid min-w-0 grid-cols-[40px_1fr] items-center gap-3">
            <span class="grid h-10 w-10 place-items-center rounded-md bg-[var(--cameroon-green)] font-black text-white">
              A
            </span>
            <span class="min-w-0">
              <strong class="block leading-tight">APMTRACK</strong>
              <small class="text-[var(--text-muted)]">Portail public</small>
            </span>
          </a>
          <nav class="flex flex-wrap gap-2 text-sm">
            <a routerLink="/public/agent" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">
              Agent
            </a>
            <a routerLink="/public/pv" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">PV</a>
            <a routerLink="/public/signalement" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">
              Signaler
            </a>
            <a routerLink="/public/signalement-suivi" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">
              Suivi
            </a>
            <a routerLink="/login" class="btn-primary">Administration</a>
          </nav>
        </div>
      </header>

      <section class="mx-auto max-w-6xl px-4 py-7">
        <router-outlet />
      </section>
    </main>
  `,
})
export class PublicShellPage {}
