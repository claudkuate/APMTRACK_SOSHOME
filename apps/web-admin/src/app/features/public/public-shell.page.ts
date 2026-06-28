import { Component, inject } from '@angular/core';
import { RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';

import { contactConfig } from '../../core/config/runtime-config';
import { I18nService } from '../../core/i18n/i18n.service';
import { TranslatePipe } from '../../core/i18n/translate.pipe';
import { HelpTipComponent } from '../../shared/ui/help-tip.component';

@Component({
  selector: 'app-public-shell-page',
  imports: [RouterLink, RouterLinkActive, RouterOutlet, TranslatePipe, HelpTipComponent],
  template: `
    <main class="relative flex min-h-screen flex-col">
      <!-- Fond : photo de Yaoundé fixe + voile clair pour la lisibilité -->
      <img
        src="/yaounde-reunification-login-hero.png"
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
              <strong class="block leading-tight">G-APM</strong>
              <small class="text-[var(--text-muted)]">APM_Tracker · {{ 'public.brand.subtitle' | t }}</small>
            </span>
          </a>
          <nav class="flex flex-wrap items-center gap-2 text-sm">
            <span class="inline-flex items-center gap-1">
              <a routerLink="/public/agent" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">
                {{ 'public.nav.agent' | t }}
              </a>
              <app-help-tip [text]="'public.why.agent' | t" />
            </span>
            <span class="inline-flex items-center gap-1">
              <a routerLink="/public/pv" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">
                {{ 'public.nav.pv' | t }}
              </a>
              <app-help-tip [text]="'public.why.pv' | t" />
            </span>
            <span class="inline-flex items-center gap-1">
              <a routerLink="/public/signalement" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">
                {{ 'public.nav.report' | t }}
              </a>
              <app-help-tip [text]="'public.why.report' | t" />
            </span>
            <a routerLink="/public/signalement-suivi" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">
              {{ 'public.nav.tracking' | t }}
            </a>
            <a routerLink="/public/a-propos" routerLinkActive="border-[var(--cameroon-green)]" class="btn-ghost">
              {{ 'public.nav.about' | t }}
            </a>
            <select
              class="rounded-md border border-[var(--line-subtle)] bg-white px-2 py-2 font-semibold"
              [value]="i18n.lang()"
              (change)="i18n.setLang($any($event.target).value)"
              [attr.aria-label]="'public.lang.aria' | t"
            >
              <option value="fr">FR</option>
              <option value="en">EN</option>
            </select>
          </nav>
        </div>
      </header>

      <section class="relative mx-auto w-full max-w-6xl flex-1 px-4 py-7">
        <router-outlet />
      </section>

      <!-- Bande contact / infoline + WhatsApp (remarques PP-02, 14bis) -->
      <footer class="border-t border-[var(--line-subtle)] bg-white">
        <div class="mx-auto flex max-w-6xl flex-wrap items-center justify-center gap-x-6 gap-y-2 px-4 py-4 text-sm">
          <span class="font-bold text-[var(--text-muted)]">{{ 'public.contact.title' | t }}</span>
          <a class="btn-ghost" [href]="'tel:' + contact.infolineTel">
            {{ 'public.contact.infoline' | t }} : {{ contact.infolinePhone }}
          </a>
          <a
            class="btn-ghost"
            [href]="'https://wa.me/' + contact.whatsappNumber"
            target="_blank"
            rel="noopener"
          >
            {{ 'public.contact.whatsapp' | t }}
          </a>
        </div>
      </footer>
    </main>
  `,
})
export class PublicShellPage {
  protected readonly i18n = inject(I18nService);
  protected readonly contact = contactConfig();
}
