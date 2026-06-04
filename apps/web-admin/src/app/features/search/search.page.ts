import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';

import { ApiService } from '../../core/services/api.service';
import { SearchResult } from '../../shared/api-types';

@Component({
  selector: 'app-search-page',
  imports: [FormsModule, RouterLink],
  template: `
    <section class="grid gap-5">
      <div>
        <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Recherche</p>
        <h2 class="text-2xl font-black">Recherche metier</h2>
        <p class="mt-1 max-w-3xl text-sm text-[var(--text-muted)]">
          Recherche serveur filtree par role et commune: agents, PV, paiements et signalements accessibles.
        </p>
      </div>

      <div class="panel grid gap-3 p-4 sm:grid-cols-[1fr_auto]">
        <div class="field">
          <label>Terme</label>
          <input [(ngModel)]="query" placeholder="Matricule, numero PV, plaque, incident, recu..." (keyup.enter)="search()" />
        </div>
        <button type="button" class="btn-primary self-end" [disabled]="loading()" (click)="search()">
          {{ loading() ? 'Recherche...' : 'Rechercher' }}
        </button>
      </div>

      @if (message()) {
        <div class="panel p-3 text-sm font-semibold text-[var(--text-muted)]">{{ message() }}</div>
      }

      <section class="grid gap-3">
        @for (result of results(); track result.module + result.id) {
          <article class="panel p-4">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <span class="status-badge">{{ result.module }}</span>
              @if (result.status) {
                <span [class]="badgeClass(result.status)">{{ humanStatus(result.status) }}</span>
              }
            </div>
            <h3 class="mt-3 font-black">{{ result.title }}</h3>
            <p class="mt-1 text-sm text-[var(--text-muted)]">{{ result.detail }}</p>
            <a class="btn-ghost mt-4 text-xs" [routerLink]="result.route">Ouvrir le module</a>
          </article>
        } @empty {
          <div class="panel p-6 text-center text-[var(--text-muted)]">
            Aucun resultat. Saisis au moins deux caracteres.
          </div>
        }
      </section>
    </section>
  `,
})
export class SearchPage {
  private readonly api = inject(ApiService);

  protected readonly results = signal<SearchResult[]>([]);
  protected readonly loading = signal(false);
  protected readonly message = signal<string | null>(null);
  protected query = '';

  protected search(): void {
    const query = this.query.trim();
    if (query.length < 2) {
      this.results.set([]);
      this.message.set('La recherche demande au moins deux caracteres.');
      return;
    }

    this.loading.set(true);
    this.message.set(null);
    this.api.get<SearchResult[]>('/api/v1/search', { q: query, limit: 30 }).subscribe({
      next: (results) => {
        this.results.set(results);
        this.loading.set(false);
        if (!results.length) {
          this.message.set('Aucun resultat accessible pour ce terme.');
        }
      },
      error: () => {
        this.loading.set(false);
        this.results.set([]);
        this.message.set('Recherche indisponible. Verifie la session ou la disponibilite API.');
      },
    });
  }

  protected badgeClass(status: string): string {
    if (['ACTIF', 'PAYE', 'TRAITE', 'NON_PAYANT'].includes(status)) {
      return 'status-badge ok';
    }
    if (['SUSPENDU', 'ANNULE', 'REJETE', 'EN_RETARD', 'RETRAITE'].includes(status)) {
      return 'status-badge danger';
    }
    return 'status-badge warn';
  }

  protected humanStatus(value: string): string {
    return value
      .toLowerCase()
      .split('_')
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(' ');
  }
}
