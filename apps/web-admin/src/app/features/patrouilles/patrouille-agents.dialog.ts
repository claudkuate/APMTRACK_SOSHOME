import { NgTemplateOutlet } from '@angular/common';
import { Component, EventEmitter, Input, OnChanges, Output, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { ApiService } from '../../core/services/api.service';
import { Paginated } from '../../shared/api-types';

interface PatrouilleAgent {
  id: string;
  patrouille_id: string;
  agent_id: string;
  role_patrouille: string;
  agent_matricule?: string;
  agent_nom?: string;
}

interface AgentOption {
  id: string;
  full_name: string;
  matricule: string;
  status: string;
}

@Component({
  selector: 'app-patrouille-agents-dialog',
  imports: [FormsModule, NgTemplateOutlet],
  template: `
    <ng-template #body>
      @if (message()) {
        <div class="panel mt-4 border-green-200 bg-green-50 p-3 text-sm font-semibold text-green-800">{{ message() }}</div>
      }
      @if (error()) {
        <div class="panel mt-4 border-red-200 bg-red-50 p-3 text-sm font-semibold text-red-800">{{ error() }}</div>
      }

      @if (locked()) {
        <p class="panel mt-4 p-3 text-sm text-[var(--text-muted)]">
          Patrouille cloturee : l'effectif n'est plus modifiable.
        </p>
      } @else {
        <div class="mt-4 grid gap-3 md:grid-cols-[1fr_180px_auto]">
          <div class="field">
            <label for="agent-select">Agent actif</label>
            <select id="agent-select" [(ngModel)]="selectedAgentId">
              <option value="">Choisir...</option>
              @for (agent of availableAgents(); track agent.id) {
                <option [value]="agent.id">{{ agent.full_name }} - {{ agent.matricule }}</option>
              }
            </select>
          </div>
          <div class="field">
            <label for="role-select">Role</label>
            <select id="role-select" [(ngModel)]="role">
              <option value="MEMBRE">Membre</option>
              <option value="CHEF">Chef</option>
            </select>
          </div>
          <div class="field justify-end">
            <label class="sr-only" for="add-agent">Ajouter</label>
            <button id="add-agent" type="button" class="btn-primary" [disabled]="!selectedAgentId || saving()" (click)="add()">
              {{ saving() ? '...' : 'Ajouter' }}
            </button>
          </div>
        </div>
      }

      <div class="mt-4 overflow-x-auto">
        <table class="data-table w-full border-collapse text-left text-sm">
          <thead>
            <tr>
              <th>Agent</th>
              <th>Matricule</th>
              <th>Role</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            @for (member of members(); track member.agent_id) {
              <tr>
                <td>{{ member.agent_nom ?? member.agent_id }}</td>
                <td>{{ member.agent_matricule ?? '-' }}</td>
                <td>
                  <span [class]="member.role_patrouille === 'CHEF' ? 'status-badge ok' : 'status-badge'">
                    {{ member.role_patrouille === 'CHEF' ? 'Chef' : 'Membre' }}
                  </span>
                </td>
                <td>
                  @if (!locked()) {
                    <button type="button" class="btn-ghost min-h-8 px-2 text-xs" (click)="remove(member.agent_id)">Retirer</button>
                  }
                </td>
              </tr>
            } @empty {
              <tr>
                <td class="px-4 py-6 text-center text-[var(--text-muted)]" colspan="4">Aucun agent affecte.</td>
              </tr>
            }
          </tbody>
        </table>
      </div>
    </ng-template>

    @if (embedded) {
      <div class="p-5">
        <ng-container [ngTemplateOutlet]="body" />
      </div>
    } @else {
      <div class="modal-backdrop" role="dialog" aria-modal="true" aria-label="Gerer les agents de la patrouille" (click)="close()">
        <div class="modal-panel modal-panel--wide" (click)="$event.stopPropagation()">
          <header class="flex flex-wrap items-start justify-between gap-3 border-b border-[var(--line-subtle)] pb-4">
            <div>
              <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Effectif patrouille</p>
              <h3 class="text-lg font-black">{{ patrouilleNom || 'Agents de la patrouille' }}</h3>
              <p class="mt-1 text-sm text-[var(--text-muted)]">
                Compose l'equipe avec des agents actifs de la commune. Un chef et des membres.
              </p>
            </div>
            <button type="button" class="btn-ghost" (click)="close()">Fermer</button>
          </header>

          <ng-container [ngTemplateOutlet]="body" />
        </div>
      </div>
    }
  `,
})
export class PatrouilleAgentsDialog implements OnChanges {
  private readonly api = inject(ApiService);

  @Input({ required: true }) patrouilleId!: string;
  @Input() communeId: string | null = null;
  @Input() patrouilleNom = '';
  @Input() patrouilleStatus = '';
  /** Rendu intégré (sans backdrop modale) pour la page de détail. */
  @Input() embedded = false;
  @Output() readonly closed = new EventEmitter<void>();

  protected readonly members = signal<PatrouilleAgent[]>([]);
  protected readonly availableAgents = signal<AgentOption[]>([]);
  protected readonly error = signal<string | null>(null);
  protected readonly message = signal<string | null>(null);
  protected readonly saving = signal(false);

  protected selectedAgentId = '';
  protected role = 'MEMBRE';

  ngOnChanges(): void {
    if (this.patrouilleId) {
      this.loadMembers();
      this.loadAvailable();
    }
  }

  protected locked(): boolean {
    return this.patrouilleStatus === 'CLOTUREE';
  }

  protected close(): void {
    this.closed.emit();
  }

  private loadMembers(): void {
    this.api.get<PatrouilleAgent[]>(`/api/v1/patrouilles/${this.patrouilleId}/agents`).subscribe({
      next: (rows) => this.members.set(rows ?? []),
      error: () => this.error.set('Chargement des agents impossible.'),
    });
  }

  private loadAvailable(): void {
    if (!this.communeId) {
      this.availableAgents.set([]);
      return;
    }
    this.api
      .page<AgentOption>('/api/v1/agents', { commune_id: this.communeId, status: 'ACTIF', page_size: 100 })
      .subscribe({
        next: (response: Paginated<AgentOption>) => this.availableAgents.set(response.items),
        error: () => this.availableAgents.set([]),
      });
  }

  protected add(): void {
    if (!this.selectedAgentId) {
      return;
    }
    this.saving.set(true);
    this.error.set(null);
    this.message.set(null);
    this.api
      .post(`/api/v1/patrouilles/${this.patrouilleId}/agents`, {
        agent_id: this.selectedAgentId,
        role_patrouille: this.role,
      })
      .subscribe({
        next: () => {
          this.saving.set(false);
          this.selectedAgentId = '';
          this.role = 'MEMBRE';
          this.message.set('Agent affecte.');
          this.loadMembers();
        },
        error: () => {
          this.saving.set(false);
          this.error.set("Affectation refusee (agent inactif, hors commune ou patrouille cloturee).");
        },
      });
  }

  protected remove(agentId: string): void {
    this.error.set(null);
    this.message.set(null);
    this.api.delete(`/api/v1/patrouilles/${this.patrouilleId}/agents/${agentId}`).subscribe({
      next: () => {
        this.message.set('Agent retire.');
        this.loadMembers();
      },
      error: () => this.error.set('Retrait refuse par l API.'),
    });
  }
}
