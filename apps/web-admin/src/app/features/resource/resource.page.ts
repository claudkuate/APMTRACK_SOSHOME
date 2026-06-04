import { Component, HostListener, OnDestroy, OnInit, inject, signal } from '@angular/core';
import { FormControl, FormGroup, FormsModule, ReactiveFormsModule, Validators } from '@angular/forms';
import { ActivatedRoute, Router, RouterLink } from '@angular/router';
import { Subscription } from 'rxjs';

import { ApiService } from '../../core/services/api.service';
import { AuthService } from '../../core/services/auth.service';
import { LookupOption, Paginated, RoleCode } from '../../shared/api-types';
import {
  RelationConfig,
  ResourceAction,
  ResourceConfig,
  ResourceField,
  ResourceFilter,
  resourceConfigs,
} from '../../shared/resource-config';

type Row = Record<string, unknown>;
type LookupState = Record<string, LookupOption[]>;

interface FormSection {
  title: string;
  fields: ResourceField[];
}

interface PendingAction {
  action: ResourceAction;
  row: Row;
}

@Component({
  selector: 'app-resource-page',
  imports: [FormsModule, ReactiveFormsModule, RouterLink],
  template: `
    @if (config(); as cfg) {
      <section class="grid gap-5">
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div>
            <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Donnees operationnelles</p>
            <h2 class="text-2xl font-black">{{ cfg.title }}</h2>
            <p class="mt-1 max-w-3xl text-sm text-[var(--text-muted)]">{{ cfg.description }}</p>
          </div>
          <div class="flex flex-wrap gap-2">
            @if (canExportCurrentRows()) {
              <button type="button" class="btn-secondary" (click)="exportCurrentRows(cfg)">Exporter</button>
            }
            @if (cfg.key === 'agents' && canMutate(cfg)) {
              <button type="button" class="btn-secondary" (click)="openImportDialog()">Importer CSV</button>
            }
            @if (canCreate(cfg)) {
              <button type="button" class="btn-primary" (click)="openForm()">
                {{ createLabel(cfg) }}
              </button>
            }
            <button type="button" class="btn-secondary" (click)="load()">Rafraichir</button>
          </div>
        </div>

        @if (message()) {
          <div class="panel border-green-200 bg-green-50 p-3 text-sm font-semibold text-green-800">{{ message() }}</div>
        }
        @if (error()) {
          <div class="panel border-red-200 bg-red-50 p-3 text-sm font-semibold text-red-800">{{ error() }}</div>
        }

        @if (showForm() && canCreate(cfg)) {
          <div class="modal-backdrop" role="dialog" aria-modal="true" [attr.aria-label]="createLabel(cfg)" (keydown.escape)="closeForm()">
            <div class="modal-panel modal-panel--wide" (click)="$event.stopPropagation()">
            <header class="flex flex-wrap items-start justify-between gap-3 border-b border-[var(--line-subtle)] pb-4">
              <div>
                <h3 class="text-lg font-black">{{ createLabel(cfg) }}</h3>
                <p class="mt-1 text-sm text-[var(--text-muted)]">
                  Les champs critiques restent valides par l'API. Les identifiants techniques sont remplaces par des listes.
                </p>
              </div>
              @if (cfg.key === 'pvs') {
                <span class="status-badge warn">Montant non modifiable</span>
              }
              <button type="button" class="btn-ghost" (click)="closeForm()">Fermer</button>
            </header>

            <form class="mt-4 grid gap-5" [formGroup]="form" (ngSubmit)="create(cfg)">
              @for (section of formSections(cfg.createFields ?? []); track section.title) {
                <fieldset class="grid gap-4">
                  <legend class="text-sm font-black text-[var(--text-strong)]">{{ section.title }}</legend>
                  <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                    @for (field of section.fields; track field.key) {
                      <div class="field" [class.field-wide]="field.type === 'textarea'">
                        <label [for]="field.key">
                          {{ field.label }}
                          @if (field.required) {
                            <span class="text-[var(--cameroon-red)]">*</span>
                          }
                        </label>

                        @if (field.type === 'textarea') {
                          <textarea [id]="field.key" [formControlName]="field.key" [placeholder]="field.placeholder ?? ''"></textarea>
                        } @else if (field.type === 'checkbox') {
                          <label class="toggle-field">
                            <input type="checkbox" [formControlName]="field.key" />
                            <span>Oui</span>
                          </label>
                        } @else if (field.type === 'relation') {
                          <select [id]="field.key" [formControlName]="field.key">
                            <option value="">Choisir...</option>
                            @for (option of optionsFor(field.key); track option.id) {
                              <option [value]="option.id">{{ optionLabel(option) }}</option>
                            }
                          </select>
                        } @else if (field.type === 'select' || field.type === 'status') {
                          <select [id]="field.key" [formControlName]="field.key">
                            <option value="">Choisir...</option>
                            @for (option of field.options ?? []; track option.value) {
                              <option [value]="option.value">{{ option.label }}</option>
                            }
                          </select>
                        } @else {
                          <input
                            [id]="field.key"
                            [type]="inputType(field)"
                            [formControlName]="field.key"
                            [placeholder]="field.placeholder ?? ''"
                          />
                        }

                        @if (field.help) {
                          <p class="field-help">{{ field.help }}</p>
                        }
                      </div>
                    }
                  </div>
                </fieldset>
              }

              @if (cfg.key === 'pvs') {
                <div class="rounded-md border border-[var(--line-subtle)] bg-[var(--surface-muted)] p-3 text-sm">
                  <strong class="block">Recapitulatif montant</strong>
                  <span class="text-[var(--text-muted)]">{{ selectedInterventionMeta() }}</span>
                </div>
              }

              <div class="flex flex-wrap items-center gap-2">
                <button type="submit" class="btn-primary" [disabled]="form.invalid || saving()">
                  {{ saving() ? 'Enregistrement...' : 'Enregistrer' }}
                </button>
                <button type="button" class="btn-secondary" (click)="closeForm()">Annuler</button>
              </div>
            </form>
            </div>
          </div>
        }

        @if (showImportDialog() && cfg.key === 'agents' && canMutate(cfg)) {
          <div class="modal-backdrop" role="dialog" aria-modal="true" aria-label="Import CSV agents" (keydown.escape)="closeImportDialog()">
            <div class="modal-panel modal-panel--wide" (click)="$event.stopPropagation()">
            <div>
              <div class="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Reprise de donnees</p>
                  <h3 class="font-black">Import CSV agents</h3>
                </div>
                <button type="button" class="btn-ghost" (click)="closeImportDialog()">Fermer</button>
              </div>
              <p class="text-sm text-[var(--text-muted)]">
                Utilise ce bloc pour une reprise initiale. Pour le quotidien, privilegie le formulaire guide.
              </p>
            </div>
            <div class="mt-4 grid gap-3 md:grid-cols-[280px_1fr]">
              <div class="field">
                <label>Commune</label>
                <select [(ngModel)]="importCommuneId">
                  <option value="">Choisir...</option>
                  @for (option of optionsFor('commune_id'); track option.id) {
                    <option [value]="option.id">{{ optionLabel(option) }}</option>
                  }
                </select>
              </div>
              <div class="field">
                <label>CSV</label>
                <textarea [(ngModel)]="importCsv"></textarea>
              </div>
            </div>
            @if (importResult()) {
              <p class="mt-3 text-sm font-semibold text-[var(--text-muted)]">{{ importResult() }}</p>
            }
            <div class="mt-5 flex flex-wrap justify-end gap-2">
              <button type="button" class="btn-secondary" (click)="closeImportDialog()">Annuler</button>
              <button type="button" class="btn-primary" (click)="importAgents()">Importer</button>
            </div>
            </div>
          </div>
        }

        <section class="panel overflow-hidden">
          <header class="grid gap-3 border-b border-[var(--line-subtle)] p-4">
            <div class="flex flex-wrap items-end justify-between gap-3">
              <div>
                <h3 class="font-black">Table de travail</h3>
                <p class="text-sm text-[var(--text-muted)]">Filtre, trie, ouvre le detail, puis applique les actions utiles.</p>
              </div>
              <span class="status-badge">{{ total() }} element(s)</span>
            </div>

            <div class="grid gap-3 lg:grid-cols-[minmax(260px,1.2fr)_repeat(3,minmax(180px,0.6fr))]">
              <div class="field">
                <label>Recherche dans la page</label>
                <input
                  [(ngModel)]="search"
                  placeholder="Numero, nom, plaque, statut..."
                  (ngModelChange)="applyTableState()"
                />
              </div>
              @for (filter of cfg.filters ?? []; track filter.key) {
                <div class="field">
                  <label>{{ filter.label }}</label>
                  @if (filter.type === 'relation') {
                    <select [(ngModel)]="filterValues[filter.key]" (ngModelChange)="filterChanged()">
                      <option value="">Tous</option>
                      @for (option of optionsFor(filter.key); track option.id) {
                        <option [value]="option.id">{{ optionLabel(option) }}</option>
                      }
                    </select>
                  } @else {
                    <select [(ngModel)]="filterValues[filter.key]" (ngModelChange)="filterChanged()">
                      <option value="">Tous</option>
                      @for (option of filter.options ?? []; track option.value) {
                        <option [value]="option.value">{{ option.label }}</option>
                      }
                    </select>
                  }
                </div>
              }
            </div>
          </header>

          <div class="overflow-x-auto">
            <table class="data-table w-full min-w-[860px] border-collapse text-left text-sm">
              <thead>
                <tr>
                  @for (column of cfg.columns; track column) {
                    <th>
                      <button type="button" class="table-sort" (click)="sort(column)">
                        {{ label(cfg, column) }}
                        <span>{{ sortIndicator(column) }}</span>
                      </button>
                    </th>
                  }
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                @for (row of visibleRows(); track row['id'] ?? row) {
                  <tr class="cursor-pointer" [class.is-selected]="selectedRow()?.['id'] === row['id']" (click)="selectRow(row)">
                    @for (column of cfg.columns; track column) {
                      <td>
                        <span [class]="badgeClass(column, row[column])">{{ display(column, row[column]) }}</span>
                      </td>
                    }
                    <td>
                      <div class="context-menu-wrap">
                        <button
                          type="button"
                          class="btn-ghost min-h-8 px-2 text-xs"
                          aria-haspopup="menu"
                          [attr.aria-expanded]="rowMenuKey() === rowKey(row)"
                          (click)="toggleRowMenu(row, $event)"
                        >
                          ...
                        </button>
                        @if (rowMenuKey() === rowKey(row)) {
                          <div
                            class="context-menu w-[190px]"
                            role="menu"
                            style="position: fixed; right: auto"
                            [style.top.px]="menuPos()?.top"
                            [style.left.px]="menuPos()?.left"
                            (click)="$event.stopPropagation()"
                          >
                            <button type="button" class="context-menu-item" role="menuitem" (click)="selectRowFromMenu(row, $event)">
                              Detail
                            </button>
                            <button type="button" class="context-menu-item" role="menuitem" (click)="exportRow(cfg, row, $event)">
                              Exporter ligne
                            </button>
                          @for (action of cfg.actions ?? []; track action.label) {
                            <button type="button" class="context-menu-item" role="menuitem" (click)="runAction(action, row, $event)">
                              {{ action.label }}
                            </button>
                          }
                          </div>
                        }
                        </div>
                      </td>
                  </tr>
                } @empty {
                  <tr>
                    <td class="px-4 py-8 text-center text-[var(--text-muted)]" [attr.colspan]="cfg.columns.length + 1">
                      Aucune donnee exploitable avec ces filtres.
                    </td>
                  </tr>
                }
              </tbody>
            </table>
          </div>

          <footer class="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--line-subtle)] px-4 py-3">
            <span class="text-sm text-[var(--text-muted)]">
              Page {{ page() }} - {{ visibleRows().length }} affiche(s)
            </span>
            <div class="flex gap-2">
              <button type="button" class="btn-secondary" [disabled]="page() <= 1" (click)="previousPage()">
                Precedent
              </button>
              <button type="button" class="btn-secondary" [disabled]="page() * pageSize() >= total()" (click)="nextPage()">
                Suivant
              </button>
            </div>
          </footer>
        </section>

        @if (selectedRow(); as row) {
          <aside class="detail-drawer" role="dialog" aria-modal="true" [attr.aria-label]="'Detail ' + rowTitle(cfg, row)" (click)="selectedRow.set(null)">
            <div class="detail-drawer__panel" (click)="$event.stopPropagation()">
              <header class="flex items-start justify-between gap-3 border-b border-[var(--line-subtle)] p-4">
                <div>
                  <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Detail</p>
                  <h3 class="text-lg font-black">{{ rowTitle(cfg, row) }}</h3>
                </div>
                <button type="button" class="btn-ghost" (click)="selectedRow.set(null)">Fermer</button>
              </header>
              <dl class="grid gap-3 p-4 sm:grid-cols-2">
                @for (field of detailFields(cfg); track field) {
                  <div class="detail-item">
                    <dt>{{ label(cfg, field) }}</dt>
                    <dd>
                      <span [class]="badgeClass(field, row[field])">{{ display(field, row[field]) }}</span>
                    </dd>
                  </div>
                }
              </dl>
            </div>
          </aside>
        }

        @if (pendingAction(); as pending) {
          <div class="modal-backdrop" role="dialog" aria-modal="true" (click)="pendingAction.set(null)">
            <div class="modal-panel" (click)="$event.stopPropagation()">
              <h3 class="text-lg font-black">{{ pending.action.confirmTitle ?? 'Confirmer cette action ?' }}</h3>
              <p class="mt-2 text-sm text-[var(--text-muted)]">
                {{ pending.action.confirmMessage ? pending.action.confirmMessage(pending.row) : 'Cette action sera appliquee immediatement.' }}
              </p>
              <div class="mt-5 flex flex-wrap justify-end gap-2">
                <button type="button" class="btn-secondary" (click)="pendingAction.set(null)">Annuler</button>
                <button type="button" class="btn-primary" (click)="confirmPendingAction()">Confirmer</button>
              </div>
            </div>
          </div>
        }
      </section>
    } @else {
      <section class="panel p-5">
        <h2 class="text-xl font-black">Module indisponible</h2>
        <p class="mt-2 text-[var(--text-muted)]">La route demandee ne correspond a aucun module du MVP.</p>
        <a routerLink="/dashboard" class="btn-primary mt-4">Retour dashboard</a>
      </section>
    }
  `,
})
export class ResourcePage implements OnInit, OnDestroy {
  private readonly route = inject(ActivatedRoute);
  private readonly router = inject(Router);
  private readonly api = inject(ApiService);
  private readonly auth = inject(AuthService);
  private subscription?: Subscription;

  protected readonly config = signal<ResourceConfig | null>(null);
  protected readonly rows = signal<Row[]>([]);
  protected readonly visibleRows = signal<Row[]>([]);
  protected readonly lookups = signal<LookupState>({});
  protected readonly error = signal<string | null>(null);
  protected readonly message = signal<string | null>(null);
  protected readonly saving = signal(false);
  protected readonly showForm = signal(false);
  protected readonly showImportDialog = signal(false);
  protected readonly page = signal(1);
  protected readonly pageSize = signal(20);
  protected readonly total = signal(0);
  protected readonly selectedRow = signal<Row | null>(null);
  protected readonly pendingAction = signal<PendingAction | null>(null);
  protected readonly importResult = signal<string | null>(null);
  protected readonly rowMenuKey = signal<string | null>(null);
  protected readonly menuPos = signal<{ top: number; left: number } | null>(null);

  protected search = '';
  protected sortKey = '';
  protected sortDirection: 'asc' | 'desc' = 'asc';
  protected filterValues: Record<string, string> = {};
  protected importCommuneId = '';
  protected importCsv = 'matricule,full_name,grade,date_prise_fonction,formation_nasla,telephone,email\n';
  protected form = new FormGroup<Record<string, FormControl<unknown>>>({});

  ngOnInit(): void {
    this.subscription = this.route.paramMap.subscribe((params) => {
      const key = params.get('feature') ?? '';
      const cfg = resourceConfigs[key] ?? null;
      this.config.set(cfg);
      this.resetPageState();
      this.buildForm(cfg);
      if (cfg) {
        this.loadLookups(cfg);
        this.load();
      }
    });
  }

  ngOnDestroy(): void {
    this.subscription?.unsubscribe();
  }

  protected openForm(): void {
    this.showForm.set(true);
    this.rowMenuKey.set(null);
  }

  protected closeForm(): void {
    this.showForm.set(false);
  }

  protected openImportDialog(): void {
    this.importResult.set(null);
    this.showImportDialog.set(true);
    this.rowMenuKey.set(null);
  }

  protected closeImportDialog(): void {
    this.showImportDialog.set(false);
  }

  protected load(): void {
    const cfg = this.config();
    if (!cfg) {
      return;
    }
    this.error.set(null);
    this.api
      .page<Row>(cfg.endpoint, {
        page: this.page(),
        page_size: this.pageSize(),
        ...(cfg.query ?? {}),
        ...this.serverFilters(cfg),
      })
      .subscribe({
        next: (response: Paginated<Row>) => {
          this.rows.set(response.items);
          this.total.set(response.total);
          this.applyTableState();
        },
        error: () => this.error.set('Chargement impossible. Verifie les droits ou la disponibilite API.'),
      });
  }

  protected create(cfg: ResourceConfig): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }
    this.saving.set(true);
    this.error.set(null);
    this.api.post<Row>(cfg.endpoint, this.formPayload(cfg.createFields ?? [])).subscribe({
      next: () => {
        this.saving.set(false);
        this.closeForm();
        this.message.set('Enregistrement effectue.');
        this.buildForm(cfg);
        this.load();
      },
      error: () => {
        this.saving.set(false);
        this.error.set('Enregistrement refuse par le backend. Controle les champs et les droits.');
      },
    });
  }

  protected importAgents(): void {
    if (!this.importCommuneId.trim() || !this.importCsv.trim()) {
      this.importResult.set('Commune et contenu CSV sont requis.');
      return;
    }
    this.api
      .postText<{ created: number; updated: number; skipped: number }>(
        '/api/v1/agents/import-csv',
        this.importCsv,
        { commune_id: this.importCommuneId.trim() },
      )
      .subscribe({
        next: (result) => {
          this.showImportDialog.set(false);
          this.message.set(`${result.created} cree(s), ${result.updated} mis a jour, ${result.skipped} ignore(s).`);
          this.load();
        },
        error: () => this.importResult.set("Import refuse. Verifie le CSV et les droits d'acces."),
      });
  }

  protected runAction(action: ResourceAction, row: Row, event?: MouseEvent): void {
    event?.stopPropagation();
    this.rowMenuKey.set(null);
    if (action.kind === 'download') {
      this.api.openDownload(action.path(row), action.filename?.(row) ?? 'document');
      return;
    }
    if (action.sensitive || action.kind === 'delete') {
      this.pendingAction.set({ action, row });
      return;
    }
    this.executeAction(action, row);
  }

  protected confirmPendingAction(): void {
    const pending = this.pendingAction();
    if (!pending) {
      return;
    }
    this.pendingAction.set(null);
    this.executeAction(pending.action, pending.row);
  }

  protected selectRow(row: Row): void {
    const cfg = this.config();
    if (cfg && this.usesDedicatedDetailPage(cfg) && row['id']) {
      this.rowMenuKey.set(null);
      this.router.navigate(['/', cfg.key, row['id']]);
      return;
    }
    this.selectedRow.set(row);
    this.rowMenuKey.set(null);
  }

  protected canMutate(cfg: ResourceConfig): boolean {
    return this.canUse(cfg.mutateRoles);
  }

  protected canCreate(cfg: ResourceConfig): boolean {
    return Boolean(cfg.createFields?.length) && this.canUse(cfg.createRoles);
  }

  protected previousPage(): void {
    if (this.page() <= 1) {
      return;
    }
    this.page.set(this.page() - 1);
    this.load();
  }

  protected nextPage(): void {
    if (this.page() * this.pageSize() >= this.total()) {
      return;
    }
    this.page.set(this.page() + 1);
    this.load();
  }

  protected filterChanged(): void {
    this.page.set(1);
    this.selectedRow.set(null);
    this.load();
  }

  protected sort(column: string): void {
    if (this.sortKey === column) {
      this.sortDirection = this.sortDirection === 'asc' ? 'desc' : 'asc';
    } else {
      this.sortKey = column;
      this.sortDirection = 'asc';
    }
    this.applyTableState();
  }

  protected toggleRowMenu(row: Row, event: MouseEvent): void {
    event.stopPropagation();
    const key = this.rowKey(row);
    if (this.rowMenuKey() === key) {
      this.rowMenuKey.set(null);
      return;
    }
    // Position the menu as a fixed overlay anchored to the trigger button so it
    // escapes the table's overflow containers and never sits under the pagination.
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const menuWidth = 190;
    const menuHeight = 132;
    const margin = 8;
    let left = rect.right - menuWidth;
    left = Math.max(margin, Math.min(left, window.innerWidth - menuWidth - margin));
    let top = rect.bottom + 4;
    if (top + menuHeight > window.innerHeight - margin) {
      top = Math.max(margin, rect.top - menuHeight - 4);
    }
    this.menuPos.set({ top, left });
    this.rowMenuKey.set(key);
  }

  @HostListener('document:click')
  protected closeRowMenu(): void {
    if (this.rowMenuKey()) {
      this.rowMenuKey.set(null);
    }
  }

  @HostListener('window:scroll')
  @HostListener('window:resize')
  protected dismissRowMenuOnViewportChange(): void {
    if (this.rowMenuKey()) {
      this.rowMenuKey.set(null);
    }
  }

  /** Escape closes the top-most open overlay (most transient first). */
  @HostListener('document:keydown.escape')
  protected dismissOnEscape(): void {
    if (this.pendingAction()) {
      this.pendingAction.set(null);
    } else if (this.showForm()) {
      this.closeForm();
    } else if (this.showImportDialog()) {
      this.closeImportDialog();
    } else if (this.selectedRow()) {
      this.selectedRow.set(null);
    } else if (this.rowMenuKey()) {
      this.rowMenuKey.set(null);
    }
  }

  protected rowKey(row: Row): string {
    return String(row['id'] ?? row['pv_id'] ?? row['receipt_number'] ?? JSON.stringify(row));
  }

  protected selectRowFromMenu(row: Row, event: MouseEvent): void {
    event.stopPropagation();
    this.selectRow(row);
  }

  protected canExportCurrentRows(): boolean {
    return this.visibleRows().length > 0;
  }

  protected exportCurrentRows(cfg: ResourceConfig): void {
    const rows = this.visibleRows();
    if (!rows.length) {
      this.message.set('Aucune donnee a exporter avec ces filtres.');
      return;
    }
    this.downloadCsv(`${cfg.key}-vue-courante.csv`, cfg, rows, this.exportColumns(cfg));
  }

  protected exportRow(cfg: ResourceConfig, row: Row, event: MouseEvent): void {
    event.stopPropagation();
    this.rowMenuKey.set(null);
    this.downloadCsv(`${cfg.key}-${this.rowKey(row)}.csv`, cfg, [row], this.detailFields(cfg));
  }

  protected sortIndicator(column: string): string {
    if (this.sortKey !== column) {
      return '';
    }
    return this.sortDirection === 'asc' ? 'ASC' : 'DESC';
  }

  protected createLabel(cfg: ResourceConfig): string {
    if (cfg.key === 'pvs') {
      return 'Nouveau PV';
    }
    if (cfg.key === 'referentiel-interventions') {
      return 'Nouvelle intervention';
    }
    return 'Nouvel element';
  }

  protected formSections(fields: ResourceField[]): FormSection[] {
    const sections = new Map<string, ResourceField[]>();
    for (const field of fields) {
      const title = field.section ?? 'Informations';
      sections.set(title, [...(sections.get(title) ?? []), field]);
    }
    return Array.from(sections.entries()).map(([title, sectionFields]) => ({ title, fields: sectionFields }));
  }

  protected optionsFor(key: string): LookupOption[] {
    return this.lookups()[key] ?? [];
  }

  protected optionLabel(option: LookupOption): string {
    return option.meta ? `${option.label} - ${option.meta}` : option.label;
  }

  protected selectedInterventionMeta(): string {
    const interventionId = String(this.form.controls['intervention_id']?.value ?? '');
    if (!interventionId) {
      return 'Choisis une intervention pour afficher le montant officiel connu du referentiel.';
    }
    const option = this.optionsFor('intervention_id').find((item) => item.id === interventionId);
    if (!option) {
      return 'Intervention selectionnee. Le backend calculera le montant officiel.';
    }
    return option.meta ? `${option.label} - ${option.meta} FCFA` : `${option.label} - montant calcule par le backend.`;
  }

  protected detailFields(cfg: ResourceConfig): string[] {
    return cfg.detailFields ?? [...cfg.columns, ...(cfg.secondaryColumns ?? [])];
  }

  protected exportColumns(cfg: ResourceConfig): string[] {
    return Array.from(new Set([...cfg.columns, ...(cfg.secondaryColumns ?? [])]));
  }

  protected rowTitle(cfg: ResourceConfig, row: Row): string {
    const preferred = ['pv_number', 'signalement_number', 'matricule', 'nom', 'full_name', 'email'];
    const key = preferred.find((candidate) => row[candidate]);
    return key ? this.display(key, row[key]) : cfg.title;
  }

  protected label(cfg: ResourceConfig, column: string): string {
    return cfg.labels[column] ?? column;
  }

  protected display(column: string, value: unknown): string {
    if (value === null || value === undefined || value === '') {
      return '-';
    }
    const relation = this.lookupLabel(column, value);
    if (relation) {
      return relation;
    }
    if (Array.isArray(value)) {
      return value.join(', ');
    }
    if (typeof value === 'boolean') {
      return value ? 'Oui' : 'Non';
    }
    if (this.isMoneyColumn(column)) {
      return `${Number(value ?? 0).toLocaleString('fr-FR')} FCFA`;
    }
    if (typeof value === 'string' && value.includes('T') && value.endsWith('Z')) {
      return new Date(value).toLocaleString('fr-FR');
    }
    if (column === 'status') {
      return this.humanStatus(String(value));
    }
    return String(value);
  }

  protected badgeClass(column: string, value: unknown): string {
    if (column !== 'status' && typeof value !== 'boolean') {
      return '';
    }
    const text = String(value);
    if (value === true || ['ACTIF', 'PAYE', 'TRAITE', 'CLOTUREE', 'NON_PAYANT'].includes(text)) {
      return 'status-badge ok';
    }
    if (['SUSPENDU', 'ANNULE', 'REJETE', 'EN_RETARD', 'RETRAITE'].includes(text)) {
      return 'status-badge danger';
    }
    return 'status-badge warn';
  }

  protected inputType(field: ResourceField): string {
    if (field.type === 'array') {
      return 'text';
    }
    if (field.type === 'money') {
      return 'number';
    }
    return field.type;
  }

  protected applyTableState(): void {
    const cfg = this.config();
    const query = this.search.trim().toLowerCase();
    let rows = this.rows().filter((row) => this.matchesLocalFilters(cfg, row));

    if (query) {
      rows = rows.filter((row) =>
        this.searchableText(row)
          .toLowerCase()
          .includes(query),
      );
    }

    if (this.sortKey) {
      rows = [...rows].sort((a, b) => {
        const left = this.display(this.sortKey, a[this.sortKey]).toLowerCase();
        const right = this.display(this.sortKey, b[this.sortKey]).toLowerCase();
        return this.sortDirection === 'asc' ? left.localeCompare(right) : right.localeCompare(left);
      });
    }

    this.visibleRows.set(rows);
  }

  private downloadCsv(filename: string, cfg: ResourceConfig, rows: Row[], columns: string[]): void {
    const header = columns.map((column) => this.csvCell(this.label(cfg, column))).join(',');
    const lines = rows.map((row) =>
      columns.map((column) => this.csvCell(this.display(column, row[column]))).join(','),
    );
    const csv = [header, ...lines].join('\n');
    const blob = new Blob([csv], { type: 'text/csv;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = filename.replace(/[^a-z0-9._-]+/gi, '-').toLowerCase();
    link.click();
    URL.revokeObjectURL(url);
  }

  private csvCell(value: string): string {
    return `"${value.replace(/"/g, '""')}"`;
  }

  private executeAction(action: ResourceAction, row: Row): void {
    const request =
      action.kind === 'delete'
        ? this.api.delete(action.path(row))
        : this.api.post(action.path(row), {});
    request.subscribe({
      next: () => {
        this.message.set('Action appliquee.');
        this.load();
      },
      error: () => this.error.set("Action refusee par l'API."),
    });
  }

  private usesDedicatedDetailPage(cfg: ResourceConfig): boolean {
    return ['pvs', 'signalements', 'communes', 'referentiel-interventions'].includes(cfg.key);
  }

  private loadLookups(cfg: ResourceConfig): void {
    const relations = this.relationsFor(cfg);
    for (const [key, relation] of relations) {
      this.api
        .page<Row>(relation.endpoint, { page_size: 100, ...(relation.query ?? {}) })
        .subscribe({
          next: (response) => {
            const options = response.items.map((row) => this.lookupOption(row, relation));
            this.lookups.update((current) => ({ ...current, [key]: options }));
          },
          error: () => {
            this.lookups.update((current) => ({ ...current, [key]: [] }));
          },
        });
    }
  }

  private relationsFor(cfg: ResourceConfig): Map<string, RelationConfig> {
    const relations = new Map<string, RelationConfig>();
    for (const field of cfg.createFields ?? []) {
      if (field.relation) {
        relations.set(field.key, field.relation);
      }
    }
    for (const filter of cfg.filters ?? []) {
      if (filter.relation) {
        relations.set(filter.key, filter.relation);
      }
    }
    return relations;
  }

  private lookupOption(row: Row, relation: RelationConfig): LookupOption {
    const valueKey = relation.valueKey ?? 'id';
    const id = String(row[valueKey] ?? '');
    return {
      id,
      label: String(row[relation.labelKey] ?? id),
      meta: row[relation.metaKey ?? ''] === undefined ? undefined : String(row[relation.metaKey ?? '']),
      status: row[relation.statusKey ?? ''] === undefined ? undefined : String(row[relation.statusKey ?? '']),
    };
  }

  private lookupLabel(column: string, value: unknown): string | null {
    const stringValue = String(value);
    const option = this.optionsFor(column).find((item) => item.id === stringValue);
    return option?.label ?? null;
  }

  private buildForm(cfg: ResourceConfig | null): void {
    const controls: Record<string, FormControl<unknown>> = {};
    for (const field of cfg?.createFields ?? []) {
      controls[field.key] = new FormControl(this.defaultValue(field), field.required ? [Validators.required] : []);
    }
    this.form = new FormGroup(controls);
  }

  private defaultValue(field: ResourceField): unknown {
    if (field.type === 'checkbox') {
      return field.key === 'active' ? true : false;
    }
    return '';
  }

  private formPayload(fields: ResourceField[]): Record<string, unknown> {
    const raw = this.form.getRawValue() as Record<string, unknown>;
    const payload: Record<string, unknown> = {};
    for (const field of fields) {
      const value = raw[field.key];
      if (field.type === 'checkbox') {
        payload[field.key] = Boolean(value);
      } else if (field.type === 'number' || field.type === 'money') {
        payload[field.key] = value === '' || value === null ? null : Number(value);
      } else if (field.type === 'array') {
        payload[field.key] = String(value ?? '')
          .split(',')
          .map((item) => item.trim())
          .filter(Boolean);
      } else if (field.key === 'roles') {
        payload[field.key] = value ? [String(value)] : [];
      } else if (value !== '' && value !== null && value !== undefined) {
        payload[field.key] = value;
      }
    }
    return payload;
  }

  private serverFilters(cfg: ResourceConfig): Record<string, string | number | boolean> {
    const query: Record<string, string | number | boolean> = {};
    for (const filter of cfg.filters ?? []) {
      const raw = this.filterValues[filter.key];
      if (!raw) {
        continue;
      }
      const queryKey = filter.queryKey ?? filter.key;
      if (raw === 'true') {
        query[queryKey] = true;
      } else if (raw === 'false') {
        query[queryKey] = false;
      } else {
        query[queryKey] = raw;
      }
    }
    return query;
  }

  private matchesLocalFilters(cfg: ResourceConfig | null, row: Row): boolean {
    for (const filter of cfg?.filters ?? []) {
      const value = this.filterValues[filter.key];
      if (!value) {
        continue;
      }
      const rowValue = row[filter.key];
      if (String(rowValue) !== value) {
        return false;
      }
    }
    return true;
  }

  private searchableText(row: Row): string {
    return Object.entries(row)
      .map(([key, value]) => `${this.display(key, value)} ${String(value ?? '')}`)
      .join(' ');
  }

  private resetPageState(): void {
    this.rows.set([]);
    this.visibleRows.set([]);
    this.lookups.set({});
    this.selectedRow.set(null);
    this.pendingAction.set(null);
    this.error.set(null);
    this.message.set(null);
    this.showForm.set(false);
    this.showImportDialog.set(false);
    this.rowMenuKey.set(null);
    this.search = '';
    this.sortKey = '';
    this.sortDirection = 'asc';
    this.filterValues = {};
    this.page.set(1);
  }

  private isMoneyColumn(column: string): boolean {
    return column.endsWith('_fcfa') || column.includes('montant') || column.includes('amount_');
  }

  private humanStatus(value: string): string {
    return value
      .toLowerCase()
      .split('_')
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(' ');
  }

  private canUse(roles: RoleCode[] | undefined): boolean {
    return !roles || this.auth.hasAnyRole(roles);
  }
}
