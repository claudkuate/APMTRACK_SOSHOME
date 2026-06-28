import { Component } from '@angular/core';

import { TranslatePipe } from '../../core/i18n/translate.pipe';

@Component({
  selector: 'app-public-missions-page',
  imports: [TranslatePipe],
  template: `
    <section class="panel grid gap-4 p-5 shadow-[var(--shadow-soft)]">
      <p class="text-xs font-bold uppercase text-[var(--text-muted)]">{{ 'public.about.eyebrow' | t }}</p>
      <h1 class="text-3xl font-black">{{ 'public.about.title' | t }}</h1>
      <p class="max-w-3xl text-[var(--text-muted)]">{{ 'public.about.intro' | t }}</p>

      <div class="grid gap-4 sm:grid-cols-2">
        <article class="rounded-md bg-[var(--surface-muted)] p-4">
          <h2 class="font-black">{{ 'public.about.missionTitle' | t }}</h2>
          <p class="mt-2 text-sm text-[var(--text-muted)]">{{ 'public.about.missionBody' | t }}</p>
        </article>
        <article class="rounded-md bg-[var(--surface-muted)] p-4">
          <h2 class="font-black">{{ 'public.about.rolesTitle' | t }}</h2>
          <p class="mt-2 text-sm text-[var(--text-muted)]">{{ 'public.about.rolesBody' | t }}</p>
        </article>
      </div>
    </section>
  `,
})
export class PublicMissionsPage {}
