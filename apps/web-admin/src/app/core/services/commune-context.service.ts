import { Injectable, computed, effect, inject, signal, untracked } from '@angular/core';

import { ApiService } from './api.service';
import { AuthService } from './auth.service';
import { DashboardSummary, Paginated } from '../../shared/api-types';

export interface Commune {
  id: string;
  code: string;
  nom: string;
  region?: string | null;
  active?: boolean;
}

const STORAGE_KEY = 'apmtrack.commune';

/**
 * Holds the active commune for tenant-scoped views.
 *
 * - Global actors (SUPER_ADMIN / global SUPERVISEUR with no commune_id) can pick any
 *   commune; the choice is appended as `commune_id` to scoped queries and persisted.
 * - Commune-bound users see their own commune read-only.
 *
 * Also exposes lightweight pending counters (signalements reçus + PV en attente)
 * derived from the dashboard summary, consumed by the sidebar badges and the bell.
 */
@Injectable({ providedIn: 'root' })
export class CommuneContextService {
  private readonly api = inject(ApiService);
  private readonly auth = inject(AuthService);

  readonly communes = signal<Commune[]>([]);
  readonly selectedId = signal<string | null>(this.readStored());

  readonly signalementsRecus = signal(0);
  readonly pvEnAttente = signal(0);
  readonly online = signal(true);

  readonly isGlobalActor = computed(() => {
    const user = this.auth.user();
    if (!user) {
      return false;
    }
    return user.commune_id === null &&
      (user.roles.includes('SUPER_ADMIN') || user.roles.includes('SUPERVISEUR'));
  });

  /** commune_id to append to scoped queries (undefined = no scoping). */
  readonly communeId = computed<string | undefined>(() => {
    const user = this.auth.user();
    if (user?.commune_id) {
      return user.commune_id;
    }
    return this.selectedId() ?? undefined;
  });

  readonly current = computed<Commune | null>(() => {
    const id = this.communeId();
    if (!id) {
      return null;
    }
    return this.communes().find((commune) => commune.id === id) ?? null;
  });

  readonly badgeCount = computed(() => this.signalementsRecus() + this.pvEnAttente());

  constructor() {
    // Reload list + counters whenever the session (user) changes.
    effect(() => {
      const user = this.auth.user();
      untracked(() => {
        if (!user) {
          this.communes.set([]);
          return;
        }
        if (user.commune_id === null &&
          (user.roles.includes('SUPER_ADMIN') || user.roles.includes('SUPERVISEUR'))) {
          this.loadCommunes();
        } else if (user.commune_id) {
          this.selectedId.set(user.commune_id);
        }
        this.refreshCounters();
      });
    });
  }

  select(id: string | null): void {
    this.selectedId.set(id);
    if (id) {
      localStorage.setItem(STORAGE_KEY, id);
    } else {
      localStorage.removeItem(STORAGE_KEY);
    }
    this.refreshCounters();
  }

  loadCommunes(): void {
    // Sélecteur global de périmètre : doit lister TOUTES les communes actives, pas les
    // 100 premières (le pays en compte ~360).
    this.api.pageAll<Commune>('/api/v1/communes', { active: true }).subscribe({
      next: (items: Commune[]) => {
        this.communes.set(items);
        const current = this.selectedId();
        if (current && !items.some((commune) => commune.id === current)) {
          this.select(null);
        }
      },
      error: () => this.communes.set([]),
    });
  }

  refreshCounters(): void {
    const communeId = this.communeId();
    this.api
      .get<DashboardSummary>('/api/v1/dashboard/summary', communeId ? { commune_id: communeId } : undefined)
      .subscribe({
        next: (summary) => {
          this.online.set(true);
          this.signalementsRecus.set(Number(summary.signalements?.['recu'] ?? 0));
          // `pending_count` couvre déjà EN_ATTENTE_PAIEMENT + EN_RETARD : le badge de
          // navigation compte ainsi exactement la même chose que le tableau de bord et
          // la caisse, au lieu d'additionner deux statuts de son côté.
          this.pvEnAttente.set(Number(summary.payments?.['pending_count'] ?? 0));
        },
        error: () => {
          this.online.set(false);
        },
      });
  }

  private readStored(): string | null {
    try {
      return localStorage.getItem(STORAGE_KEY);
    } catch {
      return null;
    }
  }
}
