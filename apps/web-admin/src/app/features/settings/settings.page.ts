import { Component, signal } from '@angular/core';
import { RouterLink } from '@angular/router';

interface SettingsLink {
  title: string;
  description: string;
  route: string;
  tone?: 'green' | 'yellow' | 'red';
}

interface SettingsTab {
  key: string;
  label: string;
  eyebrow: string;
  title: string;
  description: string;
  links: SettingsLink[];
}

@Component({
  selector: 'app-settings-page',
  imports: [RouterLink],
  template: `
    <section class="grid gap-5">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Administration</p>
          <h2 class="text-2xl font-black">Parametres</h2>
          <p class="mt-1 max-w-3xl text-sm text-[var(--text-muted)]">
            Configuration durable de la commune, du referentiel, des acces et du controle.
          </p>
        </div>
      </div>

      <nav class="settings-tabs" aria-label="Sections de parametrage">
        @for (tab of tabs; track tab.key) {
          <button
            type="button"
            class="settings-tab"
            [class.is-active]="activeTab() === tab.key"
            (click)="activeTab.set(tab.key)"
          >
            {{ tab.label }}
          </button>
        }
      </nav>

      @if (currentTab(); as tab) {
        <section class="panel overflow-hidden">
          <header class="border-b border-[var(--line-subtle)] p-5">
            <p class="text-xs font-bold uppercase text-[var(--text-muted)]">{{ tab.eyebrow }}</p>
            <h3 class="mt-1 text-xl font-black">{{ tab.title }}</h3>
            <p class="mt-1 max-w-3xl text-sm text-[var(--text-muted)]">{{ tab.description }}</p>
          </header>

          <div class="grid gap-3 p-5 md:grid-cols-2 xl:grid-cols-3">
            @for (item of tab.links; track item.route) {
              <a
                [routerLink]="item.route"
                class="settings-card"
                [class.settings-card--green]="!item.tone || item.tone === 'green'"
                [class.settings-card--yellow]="item.tone === 'yellow'"
                [class.settings-card--red]="item.tone === 'red'"
              >
                <span class="settings-card__mark" aria-hidden="true"></span>
                <span class="min-w-0">
                  <strong class="block">{{ item.title }}</strong>
                  <small class="mt-1 block text-[var(--text-muted)]">{{ item.description }}</small>
                </span>
              </a>
            }
          </div>
        </section>
      }
    </section>
  `,
})
export class SettingsPage {
  protected readonly activeTab = signal('commune');
  protected readonly tabs: SettingsTab[] = [
    {
      key: 'commune',
      label: 'Commune',
      eyebrow: 'Perimetre',
      title: 'Parametres communaux',
      description: 'Identite institutionnelle, zones de travail et rattachement territorial.',
      links: [
        {
          title: 'Communes',
          description: 'Codes, contacts, region, departement, couleur theme et etat actif.',
          route: '/communes',
          tone: 'green',
        },
        {
          title: 'Zones',
          description: 'Quartiers, secteurs, marches, zones sensibles et hierarchie locale.',
          route: '/zones',
          tone: 'yellow',
        },
      ],
    },
    {
      key: 'referentiel',
      label: 'Referentiel',
      eyebrow: 'Regles metier',
      title: 'Referentiel des interventions',
      description: 'Catalogue communal qui encadre les PV, les montants et les delais.',
      links: [
        {
          title: 'Categories',
          description: 'Premier niveau de classement du referentiel local.',
          route: '/referentiel-categories',
          tone: 'green',
        },
        {
          title: 'Types',
          description: 'Deuxieme niveau de classification des interventions.',
          route: '/referentiel-types',
          tone: 'yellow',
        },
        {
          title: 'Interventions',
          description: 'Montants, penalites, delais et references de deliberation.',
          route: '/referentiel-interventions',
          tone: 'red',
        },
      ],
    },
    {
      key: 'utilisateurs',
      label: 'Utilisateurs',
      eyebrow: 'Acces',
      title: 'Comptes et roles',
      description: 'Gestion des utilisateurs applicatifs et de leur rattachement communal.',
      links: [
        {
          title: 'Utilisateurs',
          description: 'Comptes, roles, email, activation et commune de rattachement.',
          route: '/users',
          tone: 'green',
        },
        {
          title: 'Agents',
          description: 'Profils operationnels, statut terrain et compte associe.',
          route: '/agents',
          tone: 'yellow',
        },
      ],
    },
    {
      key: 'securite',
      label: 'Securite',
      eyebrow: 'Technique',
      title: 'Etat applicatif',
      description: 'Surveillance de disponibilite et controles techniques de base.',
      links: [
        {
          title: 'Statut',
          description: 'Sante API, base de donnees et environnement courant.',
          route: '/status',
          tone: 'green',
        },
      ],
    },
    {
      key: 'audit',
      label: 'Audit',
      eyebrow: 'Controle',
      title: 'Journalisation sensible',
      description: 'Traces des actions critiques realisees dans le back-office.',
      links: [
        {
          title: 'Audit logs',
          description: 'Utilisateur, action, entite, IP, navigateur et horodatage.',
          route: '/audit-logs',
          tone: 'red',
        },
      ],
    },
  ];

  protected currentTab(): SettingsTab {
    return this.tabs.find((tab) => tab.key === this.activeTab()) ?? this.tabs[0];
  }
}
