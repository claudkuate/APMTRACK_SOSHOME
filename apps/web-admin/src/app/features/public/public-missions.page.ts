import { Component } from '@angular/core';

@Component({
  selector: 'app-public-missions-page',
  template: `
    <section class="panel grid gap-3 p-5 shadow-[var(--shadow-soft)]">
      <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Missions</p>
      <h1 class="text-3xl font-black">Roles et missions</h1>
      <p class="max-w-3xl text-[var(--text-muted)]">
        Le contenu officiel de cette rubrique sera publie apres validation par l'administration
        competente.
      </p>
    </section>
  `,
})
export class PublicMissionsPage {}
