import { Component, HostListener, OnDestroy, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { DomSanitizer, SafeHtml } from '@angular/platform-browser';
import { NavigationEnd, Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { Subscription, filter } from 'rxjs';

import { ApiService } from '../core/services/api.service';
import { AuthService } from '../core/services/auth.service';
import { CommuneContextService } from '../core/services/commune-context.service';
import { RoleCode, SearchResult } from '../shared/api-types';

type IconKey =
  | 'dashboard'
  | 'pv'
  | 'payments'
  | 'signalements'
  | 'agents'
  | 'patrouilles'
  | 'zones'
  | 'referentiel'
  | 'communes'
  | 'users'
  | 'reports'
  | 'audit'
  | 'settings';

type BadgeKey = 'payments' | 'signalements';

interface NavItem {
  label: string;
  route: string;
  icon: IconKey;
  roles?: RoleCode[];
  badge?: BadgeKey;
}

interface NavGroup {
  label: string;
  items: NavItem[];
}

const ICONS: Record<IconKey, string> = {
  dashboard:
    '<rect x="3" y="3" width="7" height="9" rx="1.5"/><rect x="14" y="3" width="7" height="5" rx="1.5"/><rect x="14" y="12" width="7" height="9" rx="1.5"/><rect x="3" y="16" width="7" height="5" rx="1.5"/>',
  pv: '<path d="M14 3v4a1 1 0 0 0 1 1h4"/><path d="M17 21H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7l5 5v11a2 2 0 0 1-2 2z"/><path d="M9 9h1M9 13h6M9 17h6"/>',
  payments:
    '<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M2 10h20"/><path d="M6 15h4"/>',
  signalements: '<path d="M4 21V4a1 1 0 0 1 1-1h11l-2 4 2 4H5"/><path d="M4 21h2"/>',
  agents:
    '<circle cx="9" cy="8" r="3"/><path d="M3 20a6 6 0 0 1 12 0"/><path d="M16 5a3 3 0 0 1 0 6"/><path d="M18 20a6 6 0 0 0-3-5.2"/>',
  patrouilles: '<path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z"/><path d="m9 12 2 2 4-4"/>',
  zones:
    '<path d="m9 4 6 2 5-2v14l-5 2-6-2-5 2V6z"/><path d="M9 4v14M15 6v14"/>',
  referentiel: '<path d="M8 6h13M8 12h13M8 18h13"/><path d="M3 6h.01M3 12h.01M3 18h.01"/>',
  communes:
    '<path d="M3 21h18"/><path d="M5 21V7l7-4 7 4v14"/><path d="M9 21v-4h6v4"/><path d="M9 10h.01M15 10h.01"/>',
  users:
    '<path d="m12 2 9 5-9 5-9-5z"/><path d="m3 12 9 5 9-5"/><path d="m3 17 9 5 9-5"/>',
  reports: '<path d="M3 3v18h18"/><rect x="7" y="11" width="3" height="6"/><rect x="13" y="7" width="3" height="10"/>',
  audit: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/><path d="M12 8v4l3 2"/>',
  settings:
    '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>',
};

@Component({
  selector: 'app-shell',
  imports: [FormsModule, RouterLink, RouterLinkActive, RouterOutlet],
  template: `
    <main class="min-h-screen bg-[var(--surface-canvas)] text-[var(--text-strong)]">
      <!-- Sidebar -->
      <aside
        class="app-sidebar fixed inset-y-0 left-0 z-30 w-64 flex-col px-3 py-4 lg:flex"
        [class.hidden]="!mobileNavOpen()"
        [class.flex]="mobileNavOpen()"
        aria-label="Navigation principale"
      >
        <a routerLink="/dashboard" class="side-brand px-2" (click)="mobileNavOpen.set(false)">
          <span class="side-emblem" aria-hidden="true">
            <img class="brand-logo" src="/armoiries-cameroun.svg" alt="" />
          </span>
          <span class="min-w-0">
            <span class="side-brand-name block truncate text-[1.05rem]">APMTRACK</span>
            <span class="side-brand-sub">Police municipale</span>
          </span>
        </a>

        <nav class="mt-6 grid flex-1 content-start gap-5 overflow-y-auto">
          @for (group of visibleGroups(); track group.label) {
            <section class="grid gap-1">
              <p class="side-group-label mb-1">{{ group.label }}</p>
              @for (item of group.items; track item.route) {
                <a
                  [routerLink]="item.route"
                  routerLinkActive="is-active"
                  class="side-link"
                  (click)="mobileNavOpen.set(false)"
                >
                  <span class="grid place-items-center" [innerHTML]="iconFor(item.icon)"></span>
                  <span class="truncate">{{ item.label }}</span>
                  @if (badgeValue(item.badge); as count) {
                    <span class="count-badge">{{ count }}</span>
                  }
                </a>
              }
            </section>
          }
        </nav>

        <div class="side-user mt-3 px-1">
          <span class="side-user-avatar" aria-hidden="true">{{ userInitials() }}</span>
          <span class="min-w-0">
            <span class="block truncate text-sm font-bold text-white">{{ user()?.full_name ?? 'Session' }}</span>
            <span class="block truncate text-xs text-[rgba(184,220,198,0.8)]">{{ roleLabel() }}</span>
          </span>
          <button
            type="button"
            class="grid h-9 w-9 place-items-center rounded-lg text-[rgba(217,230,221,0.8)] hover:bg-white/10 hover:text-white"
            title="Déconnexion"
            aria-label="Déconnexion"
            (click)="logout()"
          >
            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
              <path d="m16 17 5-5-5-5" />
              <path d="M21 12H9" />
            </svg>
          </button>
        </div>
      </aside>

      @if (mobileNavOpen()) {
        <div class="fixed inset-0 z-20 bg-[rgba(8,40,24,0.4)] lg:hidden" (click)="mobileNavOpen.set(false)"></div>
      }

      <section class="lg:pl-64">
        <!-- Topbar -->
        <header
          class="sticky top-0 z-10 border-b border-[var(--line-subtle)] bg-white/95 px-4 py-3 backdrop-blur md:px-7"
        >
          <div class="flex items-center gap-3">
            <span class="lg:hidden">
              <button
                type="button"
                class="icon-btn"
                aria-label="Ouvrir le menu"
                (click)="mobileNavOpen.set(true)"
              >
                <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
                  <path d="M4 6h16M4 12h16M4 18h16" />
                </svg>
              </button>
            </span>

            <!-- Commune switcher -->
            <div class="relative shrink-0">
              <button
                type="button"
                class="commune-switcher"
                [disabled]="!canSwitchCommune()"
                aria-haspopup="listbox"
                [attr.aria-expanded]="communeMenuOpen()"
                (click)="toggleCommuneMenu($event)"
              >
                <span class="commune-switcher__emblem" aria-hidden="true">
                  <img class="brand-logo" src="/armoiries-cameroun.svg" alt="" />
                </span>
                <span class="hidden min-w-0 text-left sm:block">
                  <span class="block max-w-44 truncate text-sm font-bold">{{ communeName() }}</span>
                  <span class="block text-[0.7rem] font-semibold uppercase tracking-wide text-[var(--text-muted)]">
                    {{ communeMeta() }}
                  </span>
                </span>
                @if (canSwitchCommune()) {
                  <svg class="text-[var(--text-muted)]" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="m6 9 6 6 6-6" />
                  </svg>
                }
              </button>

              @if (communeMenuOpen()) {
                <div class="user-menu left-0 right-auto w-72" role="listbox" (click)="$event.stopPropagation()">
                  <button
                    type="button"
                    class="user-menu-item"
                    role="option"
                    [attr.aria-selected]="!commune.communeId()"
                    (click)="selectCommune(null)"
                  >
                    Toutes les communes
                  </button>
                  @for (item of commune.communes(); track item.id) {
                    <button
                      type="button"
                      class="user-menu-item justify-between"
                      role="option"
                      [attr.aria-selected]="commune.communeId() === item.id"
                      (click)="selectCommune(item.id)"
                    >
                      <span class="min-w-0 truncate">{{ item.nom }}</span>
                      <span class="text-xs font-bold text-[var(--text-muted)]">{{ item.code }}</span>
                    </button>
                  }
                </div>
              }
            </div>

            <!-- Search -->
            <div class="topbar-search hidden md:block" (click)="$event.stopPropagation()">
              <span class="topbar-search__icon" aria-hidden="true">
                <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
                  <circle cx="11" cy="11" r="7" />
                  <path d="m21 21-4.3-4.3" />
                </svg>
              </span>
              <label class="sr-only" for="global-search">Recherche globale</label>
              <input
                id="global-search"
                [(ngModel)]="globalSearch"
                (ngModelChange)="onGlobalSearchChange($event)"
                (focus)="openSearchPanel()"
                (keyup.enter)="runGlobalSearch()"
                placeholder="Rechercher un PV, un agent, un matricule..."
              />
              @if (searchOpen()) {
                <div class="global-search-popover">
                  <div class="flex items-center justify-between gap-2 border-b border-[var(--line-subtle)] px-3 py-2">
                    <span class="text-xs font-black uppercase text-[var(--text-muted)]">Recherche</span>
                    <button type="button" class="btn-ghost min-h-8 px-2 text-xs" (click)="closeSearchPanel()">Fermer</button>
                  </div>
                  <div class="max-h-96 overflow-y-auto p-2">
                    @if (searchLoading()) {
                      <p class="px-2 py-3 text-sm text-[var(--text-muted)]">Recherche en cours...</p>
                    } @else if (searchMessage()) {
                      <p class="px-2 py-3 text-sm text-[var(--text-muted)]">{{ searchMessage() }}</p>
                    } @else {
                      @for (result of searchResults(); track result.module + result.id) {
                        <button type="button" class="search-result" (click)="openResult(result)">
                          <span class="status-badge">{{ result.module }}</span>
                          <span class="min-w-0">
                            <strong class="block truncate">{{ result.title }}</strong>
                            <small class="block truncate text-[var(--text-muted)]">{{ result.detail }}</small>
                          </span>
                          @if (result.status) {
                            <span [class]="badgeClass(result.status)">{{ humanStatus(result.status) }}</span>
                          }
                        </button>
                      }
                    }
                  </div>
                </div>
              }
            </div>

            <div class="ml-auto flex items-center gap-2">
              <span class="hidden sm:block">
                <span class="pill-online" [class.is-offline]="!commune.online()">
                  <span class="pill-online__dot" aria-hidden="true"></span>
                  {{ commune.online() ? 'En ligne' : 'Hors ligne' }}
                </span>
              </span>

              <span class="md:hidden">
                <button type="button" class="icon-btn" aria-label="Rechercher" (click)="openMobileSearch()">
                  <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
                    <circle cx="11" cy="11" r="7" />
                    <path d="m21 21-4.3-4.3" />
                  </svg>
                </button>
              </span>

              <!-- Notifications bell -->
              <div class="relative">
                <button
                  type="button"
                  class="icon-btn"
                  aria-label="Notifications"
                  [attr.aria-expanded]="bellOpen()"
                  (click)="toggleBell($event)"
                >
                  <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" />
                    <path d="M10.3 21a1.94 1.94 0 0 0 3.4 0" />
                  </svg>
                  @if (commune.badgeCount() > 0) {
                    <span class="icon-btn__dot count-badge">{{ commune.badgeCount() }}</span>
                  }
                </button>

                @if (bellOpen()) {
                  <div class="user-menu" role="menu" (click)="$event.stopPropagation()">
                    <div class="border-b border-[var(--line-subtle)] px-3 py-2">
                      <strong class="text-sm">Notifications</strong>
                    </div>
                    <a routerLink="/payments" class="user-menu-item justify-between" role="menuitem" (click)="bellOpen.set(false)">
                      <span>PV en attente de paiement</span>
                      <span class="count-badge">{{ commune.pvEnAttente() }}</span>
                    </a>
                    <a routerLink="/signalements" class="user-menu-item justify-between" role="menuitem" (click)="bellOpen.set(false)">
                      <span>Signalements reçus</span>
                      <span class="count-badge">{{ commune.signalementsRecus() }}</span>
                    </a>
                  </div>
                }
              </div>

              <span class="hidden sm:block">
                <button type="button" class="icon-btn" aria-label="Applications" disabled>
                  <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                    <circle cx="6" cy="6" r="1.6" /><circle cx="12" cy="6" r="1.6" /><circle cx="18" cy="6" r="1.6" />
                    <circle cx="6" cy="12" r="1.6" /><circle cx="12" cy="12" r="1.6" /><circle cx="18" cy="12" r="1.6" />
                    <circle cx="6" cy="18" r="1.6" /><circle cx="12" cy="18" r="1.6" /><circle cx="18" cy="18" r="1.6" />
                  </svg>
                </button>
              </span>
            </div>
          </div>
        </header>

        <div class="px-4 py-5 md:px-7 md:py-7">
          <router-outlet />
        </div>
      </section>

      @if (mobileSearchOpen()) {
        <div class="modal-backdrop" role="dialog" aria-modal="true" aria-label="Recherche globale" (keydown.escape)="closeMobileSearch()" (click)="closeMobileSearch()">
          <div class="modal-panel modal-panel--wide" (click)="$event.stopPropagation()">
            <div class="flex items-start justify-between gap-3">
              <div>
                <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Recherche globale</p>
                <h2 class="text-xl font-black">Retrouver un élément</h2>
              </div>
              <button type="button" class="btn-ghost" (click)="closeMobileSearch()">Fermer</button>
            </div>
            <div class="field mt-4">
              <label for="mobile-global-search">Terme</label>
              <input
                id="mobile-global-search"
                [(ngModel)]="globalSearch"
                (ngModelChange)="onGlobalSearchChange($event)"
                (keyup.enter)="runGlobalSearch()"
                placeholder="Matricule, numéro PV, reçu, incident..."
              />
            </div>
            <div class="mt-4 grid gap-2">
              @if (searchLoading()) {
                <p class="panel p-3 text-sm text-[var(--text-muted)]">Recherche en cours...</p>
              } @else if (searchMessage()) {
                <p class="panel p-3 text-sm text-[var(--text-muted)]">{{ searchMessage() }}</p>
              } @else {
                @for (result of searchResults(); track result.module + result.id) {
                  <button type="button" class="search-result" (click)="openResult(result)">
                    <span class="status-badge">{{ result.module }}</span>
                    <span class="min-w-0">
                      <strong class="block truncate">{{ result.title }}</strong>
                      <small class="block truncate text-[var(--text-muted)]">{{ result.detail }}</small>
                    </span>
                  </button>
                }
              }
            </div>
          </div>
        </div>
      }
    </main>
  `,
})
export class AppShellComponent implements OnDestroy {
  private readonly auth = inject(AuthService);
  private readonly api = inject(ApiService);
  private readonly router = inject(Router);
  private readonly sanitizer = inject(DomSanitizer);
  protected readonly commune = inject(CommuneContextService);
  private searchTimer: ReturnType<typeof setTimeout> | null = null;
  private readonly iconCache = new Map<IconKey, SafeHtml>();
  private readonly routerSub: Subscription;

  constructor() {
    // Any navigation closes every transient overlay so nothing lingers after leaving.
    this.routerSub = this.router.events
      .pipe(filter((event) => event instanceof NavigationEnd))
      .subscribe(() => {
        this.communeMenuOpen.set(false);
        this.bellOpen.set(false);
        this.searchOpen.set(false);
        this.mobileSearchOpen.set(false);
        this.mobileNavOpen.set(false);
      });
  }

  protected readonly user = this.auth.user;
  protected readonly searchResults = signal<SearchResult[]>([]);
  protected readonly searchLoading = signal(false);
  protected readonly searchOpen = signal(false);
  protected readonly mobileSearchOpen = signal(false);
  protected readonly mobileNavOpen = signal(false);
  protected readonly communeMenuOpen = signal(false);
  protected readonly bellOpen = signal(false);
  protected readonly searchMessage = signal<string | null>('Saisis au moins deux caractères.');
  protected globalSearch = '';

  protected readonly navGroups: NavGroup[] = [
    {
      label: 'Pilotage',
      items: [
        { label: 'Tableau de bord', route: '/dashboard', icon: 'dashboard' },
        { label: 'Procès-verbaux', route: '/pvs', icon: 'pv' },
        { label: 'Caisse & paiements', route: '/payments', icon: 'payments', badge: 'payments', roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE', 'RECEVEUR', 'SUPERVISEUR'] },
        { label: 'Signalements', route: '/signalements', icon: 'signalements', badge: 'signalements', roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE', 'SUPERVISEUR'] },
      ],
    },
    {
      label: 'Terrain',
      items: [
        { label: 'Agents APM', route: '/agents', icon: 'agents', roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE', 'SUPERVISEUR'] },
        { label: 'Patrouilles', route: '/patrouilles', icon: 'patrouilles', roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE', 'SUPERVISEUR', 'APM_AGENT'] },
        { label: 'Zones géographiques', route: '/zones', icon: 'zones', roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE', 'SUPERVISEUR'] },
      ],
    },
    {
      label: 'Référentiels',
      items: [
        { label: 'Référentiel', route: '/referentiel-interventions', icon: 'referentiel', roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE', 'SUPERVISEUR'] },
        { label: 'Communes', route: '/communes', icon: 'communes', roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE', 'SUPERVISEUR'] },
        { label: 'Utilisateurs système', route: '/users', icon: 'users', roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'] },
      ],
    },
    {
      label: 'Supervision',
      items: [
        { label: 'Rapports', route: '/exports', icon: 'reports', roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE', 'SUPERVISEUR', 'RECEVEUR'] },
        { label: "Journal d'audit", route: '/audit-logs', icon: 'audit', roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE', 'SUPERVISEUR'] },
        { label: 'Paramètres', route: '/settings', icon: 'settings' },
      ],
    },
  ];

  protected readonly visibleGroups = computed(() =>
    this.navGroups
      .map((group) => ({
        ...group,
        items: group.items.filter((item) => !item.roles || this.auth.hasAnyRole(item.roles)),
      }))
      .filter((group) => group.items.length > 0),
  );

  protected iconFor(key: IconKey): SafeHtml {
    let cached = this.iconCache.get(key);
    if (!cached) {
      cached = this.sanitizer.bypassSecurityTrustHtml(
        `<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${ICONS[key]}</svg>`,
      );
      this.iconCache.set(key, cached);
    }
    return cached;
  }

  protected badgeValue(badge?: BadgeKey): number | null {
    if (!badge) {
      return null;
    }
    const value = badge === 'payments' ? this.commune.pvEnAttente() : this.commune.signalementsRecus();
    return value > 0 ? value : null;
  }

  protected canSwitchCommune(): boolean {
    return this.commune.isGlobalActor();
  }

  protected communeName(): string {
    const current = this.commune.current();
    if (current) {
      return `Commune de ${current.nom}`;
    }
    return this.canSwitchCommune() ? 'Toutes les communes' : 'Périmètre communal';
  }

  protected communeMeta(): string {
    const current = this.commune.current();
    if (current) {
      return `${current.code}${current.region ? ' · ' + current.region : ''}`;
    }
    return this.canSwitchCommune() ? 'Vue globale' : 'Accès restreint';
  }

  protected toggleCommuneMenu(event: MouseEvent): void {
    event.stopPropagation();
    if (!this.canSwitchCommune()) {
      return;
    }
    this.communeMenuOpen.set(!this.communeMenuOpen());
    this.bellOpen.set(false);
    this.searchOpen.set(false);
  }

  protected selectCommune(id: string | null): void {
    this.commune.select(id);
    this.communeMenuOpen.set(false);
  }

  protected toggleBell(event: MouseEvent): void {
    event.stopPropagation();
    this.bellOpen.set(!this.bellOpen());
    this.communeMenuOpen.set(false);
    this.searchOpen.set(false);
  }

  /** Close transient menus when a click lands anywhere outside their triggers/panels. */
  @HostListener('document:click')
  protected closeOverlays(): void {
    this.communeMenuOpen.set(false);
    this.bellOpen.set(false);
    this.searchOpen.set(false);
  }

  /** Escape closes any open transient UI. */
  @HostListener('document:keydown.escape')
  protected dismissAll(): void {
    this.communeMenuOpen.set(false);
    this.bellOpen.set(false);
    this.searchOpen.set(false);
    this.mobileSearchOpen.set(false);
    this.mobileNavOpen.set(false);
  }

  ngOnDestroy(): void {
    if (this.searchTimer) {
      clearTimeout(this.searchTimer);
    }
    this.routerSub?.unsubscribe();
  }

  protected onGlobalSearchChange(value: string): void {
    this.globalSearch = value;
    this.openSearchPanel();
    if (this.searchTimer) {
      clearTimeout(this.searchTimer);
    }
    const query = value.trim();
    if (query.length < 2) {
      this.searchResults.set([]);
      this.searchLoading.set(false);
      this.searchMessage.set('Saisis au moins deux caractères.');
      return;
    }
    this.searchLoading.set(true);
    this.searchMessage.set(null);
    this.searchTimer = setTimeout(() => this.runGlobalSearch(), 260);
  }

  protected runGlobalSearch(): void {
    const query = this.globalSearch.trim();
    if (query.length < 2) {
      this.searchResults.set([]);
      this.searchLoading.set(false);
      this.searchMessage.set('Saisis au moins deux caractères.');
      return;
    }
    this.searchLoading.set(true);
    this.api.get<SearchResult[]>('/api/v1/search', { q: query, limit: 8 }).subscribe({
      next: (results) => {
        this.searchResults.set(results);
        this.searchLoading.set(false);
        this.searchMessage.set(results.length ? null : 'Aucun résultat accessible.');
      },
      error: () => {
        this.searchResults.set([]);
        this.searchLoading.set(false);
        this.searchMessage.set('Recherche indisponible.');
      },
    });
  }

  protected openSearchPanel(): void {
    this.searchOpen.set(true);
  }

  protected closeSearchPanel(): void {
    this.searchOpen.set(false);
  }

  protected openMobileSearch(): void {
    this.mobileSearchOpen.set(true);
    this.openSearchPanel();
  }

  protected closeMobileSearch(): void {
    this.mobileSearchOpen.set(false);
    this.closeSearchPanel();
  }

  protected openResult(result: SearchResult): void {
    this.searchOpen.set(false);
    this.mobileSearchOpen.set(false);
    this.router.navigateByUrl(result.route || '/dashboard');
  }

  protected userInitials(): string {
    const value = this.user()?.full_name || this.user()?.email || 'APMTRACK';
    return value
      .split(/[ .@_-]+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((part) => part.charAt(0).toUpperCase())
      .join('');
  }

  protected roleLabel(): string {
    const role = this.user()?.roles?.[0];
    const labels: Record<RoleCode, string> = {
      SUPER_ADMIN: 'Super administrateur',
      ADMIN_COMMUNE: 'Administrateur',
      APM_AGENT: 'Agent APM',
      SUPERVISEUR: 'Superviseur',
      RECEVEUR: 'Receveur municipal',
    };
    return role ? labels[role] : 'Session';
  }

  protected badgeClass(status: string): string {
    if (['ACTIF', 'PAYE', 'TRAITE', 'NON_PAYANT', 'CLOTUREE'].includes(status)) {
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

  protected logout(): void {
    this.auth.logout().subscribe(() => this.router.navigateByUrl('/login'));
  }
}
