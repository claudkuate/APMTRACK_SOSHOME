import { Component, HostListener, OnDestroy, OnInit, inject, signal } from '@angular/core';
import {
  FormControl,
  FormGroup,
  FormsModule,
  ReactiveFormsModule,
  Validators,
} from '@angular/forms';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { Subscription, firstValueFrom } from 'rxjs';

import { ApiService } from '../../core/services/api.service';
import { AuthService } from '../../core/services/auth.service';
import { I18nService } from '../../core/i18n/i18n.service';
import { AutoTranslatePipe } from '../../core/i18n/auto-translate.pipe';
import { LookupService } from '../../core/services/lookup.service';
import { LookupOption, Paginated, RoleCode } from '../../shared/api-types';
import { downloadCsv as saveCsv } from '../../shared/csv';
import { describeHttpError } from '../../shared/http-error';
import {
  RelationConfig,
  ResourceAction,
  ResourceConfig,
  ResourceField,
  ResourceFilter,
  SelectOption,
  resourceConfigs,
} from '../../shared/resource-config';
import { PatrouilleAgentsDialog } from '../patrouilles/patrouille-agents.dialog';
import { LocationPickerComponent } from '../../shared/map/location-picker.component';
import { ZoneEditorComponent } from '../../shared/map/zone-editor.component';
import { GeoGeometry } from '../../core/services/geo.service';

type Row = Record<string, unknown>;

interface FormSection {
  title: string;
  fields: ResourceField[];
}

interface PendingAction {
  action: ResourceAction;
  row: Row;
}

interface StatusContext {
  action: ResourceAction;
  row: Row;
  options: SelectOption[];
  current: string;
}

interface AgentsDialogContext {
  patrouilleId: string;
  communeId: string | null;
  nom: string;
  status: string;
}

@Component({
  selector: 'app-resource-page',
  imports: [
    FormsModule,
    ReactiveFormsModule,
    RouterLink,
    PatrouilleAgentsDialog,
    LocationPickerComponent,
    ZoneEditorComponent,
    AutoTranslatePipe,
  ],
  template: `
    @if (config(); as cfg) {
      <section class="grid gap-5">
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div>
            <p class="text-xs font-bold uppercase text-[var(--text-muted)]">
              {{ 'Donnees operationnelles' | auto }}
            </p>
            <h2 class="text-2xl font-black">{{ cfg.title | auto }}</h2>
            <p class="mt-1 max-w-3xl text-sm text-[var(--text-muted)]">{{ cfg.description | auto }}</p>
          </div>
          <div class="flex flex-wrap gap-2">
            @if (canExportCurrentRows()) {
              <button type="button" class="btn-secondary" (click)="exportCurrentRows(cfg)">
                {{ 'Exporter' | auto }}
              </button>
            }
            @if (cfg.key === 'agents' && canMutate(cfg)) {
              <button type="button" class="btn-secondary" (click)="openImportDialog()">
                {{ 'Importer CSV' | auto }}
              </button>
            }
            @if (canCreate(cfg)) {
              <button type="button" class="btn-primary" (click)="openForm()">
                {{ createLabel(cfg) }}
              </button>
            }
            <button type="button" class="btn-secondary" (click)="load()">{{ 'Rafraichir' | auto }}</button>
          </div>
        </div>

        @if (message()) {
          <div class="panel border-green-200 bg-green-50 p-3 text-sm font-semibold text-green-800">
            {{ message() }}
          </div>
        }
        @if (error()) {
          <div class="panel border-red-200 bg-red-50 p-3 text-sm font-semibold text-red-800">
            {{ error() }}
          </div>
        }

        @if (showForm() && canOpenForm(cfg)) {
          <div
            class="modal-backdrop"
            role="dialog"
            aria-modal="true"
            [attr.aria-label]="formTitle(cfg)"
            (keydown.escape)="closeForm()"
          >
            <div class="modal-panel modal-panel--wide" (click)="$event.stopPropagation()">
              <header
                class="flex flex-wrap items-start justify-between gap-3 border-b border-[var(--line-subtle)] pb-4"
              >
                <div>
                  <h3 class="text-lg font-black">{{ formTitle(cfg) }}</h3>
                </div>
                @if (cfg.key === 'pvs') {
                  <span class="status-badge warn">{{ 'Montant non modifiable' | auto }}</span>
                }
                <button type="button" class="btn-ghost" (click)="closeForm()">{{ 'Fermer' | auto }}</button>
              </header>

              <form class="mt-4 grid gap-5" [formGroup]="form" (ngSubmit)="submit(cfg)">
                @for (section of formSections(effectiveFields(cfg)); track section.title) {
                  <fieldset class="grid gap-4">
                    <legend class="text-sm font-black text-[var(--text-strong)]">
                      {{ section.title | auto }}
                    </legend>
                    <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                      @for (field of section.fields; track field.key) {
                        <div class="field" [class.field-wide]="isWideField(field)">
                          <label [for]="field.key">
                            {{ field.label | auto }}
                            @if (field.required) {
                              <span class="text-[var(--cameroon-red)]">*</span>
                            }
                          </label>

                          @if (field.type === 'geopoint') {
                            <app-location-picker
                              [latitude]="geoNumber(field.latKey ?? field.key)"
                              (latitudeChange)="setControl(field.latKey ?? field.key, $event)"
                              [longitude]="geoNumber(field.lonKey ?? 'gps_longitude')"
                              (longitudeChange)="
                                setControl(field.lonKey ?? 'gps_longitude', $event)
                              "
                              (addressResolved)="onAddressResolved($event)"
                            />
                          } @else if (field.type === 'geopolygon') {
                            <app-zone-editor
                              [boundary]="geoBoundary(field.key)"
                              (boundaryChange)="setControl(field.key, $event)"
                              [layer]="cfg.key === 'communes' ? 'communes' : 'zones'"
                            />
                          } @else if (field.type === 'textarea') {
                            <textarea
                              [id]="field.key"
                              [formControlName]="field.key"
                              [placeholder]="field.placeholder ?? ''"
                            ></textarea>
                          } @else if (field.type === 'checkbox') {
                            <label class="toggle-field">
                              <input type="checkbox" [formControlName]="field.key" />
                              <span>{{ 'Oui' | auto }}</span>
                            </label>
                          } @else if (field.type === 'relation_multi') {
                            <select [id]="field.key" [formControlName]="field.key" multiple>
                              @for (option of optionsForField(field); track option.id) {
                                <option [value]="option.id">{{ optionLabel(option) }}</option>
                              }
                            </select>
                          } @else if (field.type === 'relation') {
                            <select [id]="field.key" [formControlName]="field.key">
                              <option value="">{{ 'Choisir...' | auto }}</option>
                              @for (option of optionsForField(field); track option.id) {
                                <option [value]="option.id">{{ optionLabel(option) }}</option>
                              }
                            </select>
                          } @else if (field.type === 'select_multi') {
                            <select [id]="field.key" [formControlName]="field.key" multiple>
                              @for (option of field.options ?? []; track option.value) {
                                <option [value]="option.value">{{ option.label | auto }}</option>
                              }
                            </select>
                          } @else if (field.type === 'select' || field.type === 'status') {
                            <select [id]="field.key" [formControlName]="field.key">
                              <option value="">{{ 'Choisir...' | auto }}</option>
                              @for (option of field.options ?? []; track option.value) {
                                <option [value]="option.value">{{ option.label | auto }}</option>
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
                            <p class="field-help">{{ field.help | auto }}</p>
                          }
                        </div>
                      }
                    </div>
                  </fieldset>
                }

                @if (cfg.key === 'pvs') {
                  <div
                    class="rounded-md border border-[var(--line-subtle)] bg-[var(--surface-muted)] p-3 text-sm"
                  >
                    <strong class="block">{{ 'Recapitulatif montant' | auto }}</strong>
                    <span class="text-[var(--text-muted)]">{{ selectedInterventionMeta() }}</span>
                  </div>
                }

                <div class="flex flex-wrap items-center gap-2">
                  <button type="submit" class="btn-primary" [disabled]="form.invalid || saving()">
                    {{ (saving() ? 'Enregistrement...' : 'Enregistrer') | auto }}
                  </button>
                  <button type="button" class="btn-secondary" (click)="closeForm()">{{ 'Annuler' | auto }}</button>
                </div>
              </form>
            </div>
          </div>
        }

        @if (showImportDialog() && cfg.key === 'agents' && canMutate(cfg)) {
          <div
            class="modal-backdrop"
            role="dialog"
            aria-modal="true"
            aria-label="Import CSV agents"
            (keydown.escape)="closeImportDialog()"
          >
            <div class="modal-panel modal-panel--wide" (click)="$event.stopPropagation()">
              <div>
                <div class="flex flex-wrap items-start justify-between gap-3">
                  <div>
                    <p class="text-xs font-bold uppercase text-[var(--text-muted)]">
                      Reprise de donnees
                    </p>
                    <h3 class="font-black">Import CSV agents</h3>
                  </div>
                  <button type="button" class="btn-ghost" (click)="closeImportDialog()">
                    Fermer
                  </button>
                </div>
                <p class="text-sm text-[var(--text-muted)]">
                  Utilise ce bloc pour une reprise initiale. Pour le quotidien, privilegie le
                  formulaire guide.
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
                <p class="mt-3 text-sm font-semibold text-[var(--text-muted)]">
                  {{ importResult() }}
                </p>
              }
              <div class="mt-5 flex flex-wrap justify-end gap-2">
                <button type="button" class="btn-secondary" (click)="closeImportDialog()">
                  Annuler
                </button>
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
                <p class="text-sm text-[var(--text-muted)]">
                  Filtre, trie, ouvre le detail, puis applique les actions utiles.
                </p>
              </div>
              <span class="status-badge">{{ total() }} element(s)</span>
            </div>

            <div
              class="grid gap-3 lg:grid-cols-[minmax(260px,1.2fr)_repeat(3,minmax(180px,0.6fr))]"
            >
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
                    <select
                      [(ngModel)]="filterValues[filter.key]"
                      (ngModelChange)="filterChanged()"
                    >
                      <option value="">Tous</option>
                      @for (option of optionsFor(filter.key); track option.id) {
                        <option [value]="option.id">{{ optionLabel(option) }}</option>
                      }
                    </select>
                  } @else {
                    <select
                      [(ngModel)]="filterValues[filter.key]"
                      (ngModelChange)="filterChanged()"
                    >
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
                  <th>{{ 'Actions' | auto }}</th>
                </tr>
              </thead>
              <tbody>
                @for (row of visibleRows(); track row['id'] ?? row) {
                  <tr
                    class="cursor-pointer"
                    [class.is-selected]="selectedRow()?.['id'] === row['id']"
                    (click)="selectRow(row)"
                  >
                    @for (column of cfg.columns; track column; let i = $index) {
                      <td>
                        @if (i === 0 && row['id']) {
                          <div class="flex items-center gap-2.5">
                            @if (cfg.photoEndpoint) {
                              <span
                                class="grid h-8 w-8 flex-none place-items-center overflow-hidden rounded-full border border-[var(--line-subtle)] bg-[var(--surface-muted)] text-[0.65rem] font-bold text-[var(--text-muted)]"
                              >
                                @if (avatarFor(row); as url) {
                                  <img [src]="url" alt="" class="h-full w-full object-cover" />
                                } @else {
                                  {{ initials(rowTitle(cfg, row)) }}
                                }
                              </span>
                            }
                            <a
                              class="font-semibold text-[var(--cameroon-red)] hover:underline"
                              [routerLink]="['/', cfg.key, row['id']]"
                              (click)="$event.stopPropagation()"
                              >{{ display(column, row[column]) }}</a
                            >
                          </div>
                        } @else {
                          <span [class]="badgeClass(column, row[column])">{{
                            display(column, row[column])
                          }}</span>
                        }
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
                            <button
                              type="button"
                              class="context-menu-item"
                              role="menuitem"
                              (click)="selectRowFromMenu(row, $event)"
                            >
                              {{ 'Detail' | auto }}
                            </button>
                            @if (canMutate(cfg) && hasEditableFields(cfg)) {
                              <button
                                type="button"
                                class="context-menu-item"
                                role="menuitem"
                                (click)="openEditFromMenu(cfg, row, $event)"
                              >
                                {{ 'Editer' | auto }}
                              </button>
                            }
                            @if (cfg.manageAgents && canMutate(cfg)) {
                              <button
                                type="button"
                                class="context-menu-item"
                                role="menuitem"
                                (click)="openAgentsFromMenu(cfg, row, $event)"
                              >
                                {{ 'Gerer les agents' | auto }}
                              </button>
                            }
                            <button
                              type="button"
                              class="context-menu-item"
                              role="menuitem"
                              (click)="exportRow(cfg, row, $event)"
                            >
                              {{ 'Exporter ligne' | auto }}
                            </button>
                            @for (action of visibleActions(cfg); track action.label) {
                              <button
                                type="button"
                                class="context-menu-item"
                                role="menuitem"
                                (click)="runAction(action, row, $event)"
                              >
                                {{ action.label | auto }}
                              </button>
                            }
                          </div>
                        }
                      </div>
                    </td>
                  </tr>
                } @empty {
                  <tr>
                    <td
                      class="px-4 py-8 text-center text-[var(--text-muted)]"
                      [attr.colspan]="cfg.columns.length + 1"
                    >
                      {{ 'Aucune donnee exploitable avec ces filtres.' | auto }}
                    </td>
                  </tr>
                }
              </tbody>
            </table>
          </div>

          <footer
            class="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--line-subtle)] px-4 py-3"
          >
            <span class="text-sm text-[var(--text-muted)]">
              Page {{ page() }} - {{ visibleRows().length }} affiche(s)
            </span>
            <div class="flex gap-2">
              <button
                type="button"
                class="btn-secondary"
                [disabled]="page() <= 1"
                (click)="previousPage()"
              >
                Precedent
              </button>
              <button
                type="button"
                class="btn-secondary"
                [disabled]="page() * pageSize() >= total()"
                (click)="nextPage()"
              >
                Suivant
              </button>
            </div>
          </footer>
        </section>

        @if (selectedRow(); as row) {
          <aside
            class="detail-drawer"
            role="dialog"
            aria-modal="true"
            [attr.aria-label]="'Detail ' + rowTitle(cfg, row)"
            (click)="selectedRow.set(null)"
          >
            <div class="detail-drawer__panel" (click)="$event.stopPropagation()">
              <header
                class="flex items-start justify-between gap-3 border-b border-[var(--line-subtle)] p-4"
              >
                <div>
                  <p class="text-xs font-bold uppercase text-[var(--text-muted)]">Detail</p>
                  <h3 class="text-lg font-black">{{ rowTitle(cfg, row) }}</h3>
                </div>
                <div class="flex items-center gap-2">
                  @if (row['id']) {
                    <a
                      class="btn-secondary"
                      [routerLink]="['/', cfg.key, row['id']]"
                      (click)="selectedRow.set(null)"
                    >
                      Voir la fiche
                    </a>
                  }
                  @if (canMutate(cfg) && hasEditableFields(cfg)) {
                    <button type="button" class="btn-secondary" (click)="openEdit(cfg, row)">
                      Editer
                    </button>
                  }
                  <button type="button" class="btn-ghost" (click)="selectedRow.set(null)">
                    Fermer
                  </button>
                </div>
              </header>
              <dl class="grid gap-3 p-4 sm:grid-cols-2">
                @for (field of detailFields(cfg); track field) {
                  <div class="detail-item">
                    <dt>{{ label(cfg, field) }}</dt>
                    <dd>
                      <span [class]="badgeClass(field, row[field])">{{
                        display(field, row[field])
                      }}</span>
                    </dd>
                  </div>
                }
              </dl>
            </div>
          </aside>
        }

        @if (pendingAction(); as pending) {
          <div
            class="modal-backdrop"
            role="dialog"
            aria-modal="true"
            (click)="pendingAction.set(null)"
          >
            <div class="modal-panel" (click)="$event.stopPropagation()">
              <h3 class="text-lg font-black">
                {{ pending.action.confirmTitle ?? 'Confirmer cette action ?' }}
              </h3>
              <p class="mt-2 text-sm text-[var(--text-muted)]">
                {{
                  pending.action.confirmMessage
                    ? pending.action.confirmMessage(pending.row)
                    : 'Cette action sera appliquee immediatement.'
                }}
              </p>
              <div class="mt-5 flex flex-wrap justify-end gap-2">
                <button type="button" class="btn-secondary" (click)="pendingAction.set(null)">
                  Annuler
                </button>
                <button type="button" class="btn-primary" (click)="confirmPendingAction()">
                  Confirmer
                </button>
              </div>
            </div>
          </div>
        }

        @if (statusContext(); as ctx) {
          <div
            class="modal-backdrop"
            role="dialog"
            aria-modal="true"
            [attr.aria-label]="ctx.action.label"
            (keydown.escape)="statusContext.set(null)"
            (click)="statusContext.set(null)"
          >
            <div class="modal-panel" (click)="$event.stopPropagation()">
              <h3 class="text-lg font-black">{{ ctx.action.label }}</h3>
              <p class="mt-1 text-sm text-[var(--text-muted)]">
                {{ ctx.action.currentLabel ?? 'Statut courant' }} :
                <span [class]="badgeClass('status', ctx.current)">{{
                  display('status', ctx.current)
                }}</span>
              </p>
              @if (ctx.options.length === 0) {
                <p class="panel mt-4 p-3 text-sm text-[var(--text-muted)]">
                  Aucune transition disponible depuis ce statut.
                </p>
                <div class="mt-5 flex justify-end">
                  <button type="button" class="btn-secondary" (click)="statusContext.set(null)">
                    Fermer
                  </button>
                </div>
              } @else {
                <form
                  class="mt-4 grid gap-4"
                  [formGroup]="statusForm"
                  (ngSubmit)="submitStatus(ctx)"
                >
                  <div class="field">
                    <label for="status-target"
                      >{{ ctx.action.selectLabel ?? 'Nouveau statut' }}
                      <span class="text-[var(--cameroon-red)]">*</span></label
                    >
                    <select id="status-target" formControlName="status">
                      <option value="">Choisir...</option>
                      @for (opt of ctx.options; track opt.value) {
                        <option [value]="opt.value">{{ opt.label }}</option>
                      }
                    </select>
                  </div>
                  @for (extra of ctx.action.statusExtra ?? []; track extra.key) {
                    <div class="field">
                      <label [for]="'status-extra-' + extra.key">{{ extra.label }}</label>
                      @if (extra.type === 'textarea') {
                        <textarea
                          [id]="'status-extra-' + extra.key"
                          [formControlName]="extra.key"
                          [placeholder]="extra.placeholder ?? ''"
                        ></textarea>
                      } @else if (extra.type === 'relation') {
                        <select [id]="'status-extra-' + extra.key" [formControlName]="extra.key">
                          <option value="">Choisir...</option>
                          @for (option of optionsFor(extra.key); track option.id) {
                            <option [value]="option.id">{{ optionLabel(option) }}</option>
                          }
                        </select>
                      } @else {
                        <input
                          [id]="'status-extra-' + extra.key"
                          type="text"
                          [formControlName]="extra.key"
                          [placeholder]="extra.placeholder ?? ''"
                        />
                      }
                    </div>
                  }
                  <div class="flex flex-wrap justify-end gap-2">
                    <button type="button" class="btn-secondary" (click)="statusContext.set(null)">
                      Annuler
                    </button>
                    <button
                      type="submit"
                      class="btn-primary"
                      [disabled]="statusForm.invalid || saving()"
                    >
                      {{ saving() ? 'Application...' : 'Appliquer' }}
                    </button>
                  </div>
                </form>
              }
            </div>
          </div>
        }

        @if (agentsDialog(); as ctx) {
          <app-patrouille-agents-dialog
            [patrouilleId]="ctx.patrouilleId"
            [communeId]="ctx.communeId"
            [patrouilleNom]="ctx.nom"
            [patrouilleStatus]="ctx.status"
            (closed)="agentsDialog.set(null)"
          />
        }
      </section>
    } @else {
      <section class="panel p-5">
        <h2 class="text-xl font-black">Module indisponible</h2>
        <p class="mt-2 text-[var(--text-muted)]">
          La route demandee ne correspond a aucun module du MVP.
        </p>
        <a routerLink="/dashboard" class="btn-primary mt-4">Retour dashboard</a>
      </section>
    }
  `,
})
export class ResourcePage implements OnInit, OnDestroy {
  private readonly route = inject(ActivatedRoute);
  private readonly api = inject(ApiService);
  private readonly auth = inject(AuthService);
  private readonly lookup = inject(LookupService);
  protected readonly i18n = inject(I18nService);
  private subscription?: Subscription;
  /** Champs du formulaire courant (pour l'affichage conditionnel `visibleWhen`). */
  private formFields: ResourceField[] = [];
  private conditionalSub?: Subscription;

  protected readonly config = signal<ResourceConfig | null>(null);
  protected readonly rows = signal<Row[]>([]);
  protected readonly visibleRows = signal<Row[]>([]);
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
  protected readonly formMode = signal<'create' | 'edit'>('create');
  protected readonly editingId = signal<string | null>(null);
  protected readonly statusContext = signal<StatusContext | null>(null);
  protected readonly agentsDialog = signal<AgentsDialogContext | null>(null);
  /** id de ligne → object URL de l'avatar (résolu via blob authentifié). */
  protected readonly avatarUrls = signal<Record<string, string>>({});

  protected search = '';
  protected sortKey = '';
  protected sortDirection: 'asc' | 'desc' = 'asc';
  protected filterValues: Record<string, string> = {};
  protected importCommuneId = '';
  protected importCsv = 'matricule,full_name,date_prise_fonction,telephone,email\n';
  protected form = new FormGroup<Record<string, FormControl<unknown>>>({});
  protected statusForm = new FormGroup<Record<string, FormControl<unknown>>>({});

  ngOnInit(): void {
    this.subscription = this.route.paramMap.subscribe((params) => {
      const key = params.get('feature') ?? '';
      const cfg = resourceConfigs[key] ?? null;
      this.config.set(cfg);
      this.resetPageState();
      this.buildForm(cfg?.createFields ?? []);
      if (cfg) {
        this.lookup.reset();
        this.lookup.loadForConfig(cfg);
        this.load();
      }
    });
  }

  ngOnDestroy(): void {
    this.subscription?.unsubscribe();
    this.conditionalSub?.unsubscribe();
    this.clearAvatars();
  }

  protected openForm(): void {
    const cfg = this.config();
    this.formMode.set('create');
    this.editingId.set(null);
    this.buildForm(cfg?.createFields ?? []);
    this.showForm.set(true);
    this.rowMenuKey.set(null);
  }

  protected openEdit(cfg: ResourceConfig, row: Row): void {
    const fields = cfg.patchFields ?? cfg.createFields ?? [];
    this.formMode.set('edit');
    this.editingId.set(String(row['id'] ?? ''));
    this.buildForm(fields);
    this.patchForm(fields, row);
    this.selectedRow.set(null);
    this.showForm.set(true);
    this.rowMenuKey.set(null);
  }

  protected openEditFromMenu(cfg: ResourceConfig, row: Row, event: MouseEvent): void {
    event.stopPropagation();
    this.openEdit(cfg, row);
  }

  protected openAgentsFromMenu(cfg: ResourceConfig, row: Row, event: MouseEvent): void {
    event.stopPropagation();
    this.rowMenuKey.set(null);
    this.agentsDialog.set({
      patrouilleId: String(row['id'] ?? ''),
      communeId: row['commune_id'] ? String(row['commune_id']) : null,
      nom: String(row['nom'] ?? 'Patrouille'),
      status: String(row['status'] ?? ''),
    });
  }

  protected closeForm(): void {
    this.showForm.set(false);
    this.formMode.set('create');
    this.editingId.set(null);
  }

  protected effectiveFields(cfg: ResourceConfig): ResourceField[] {
    if (this.formMode() === 'edit') {
      return cfg.patchFields ?? cfg.createFields ?? [];
    }
    return cfg.createFields ?? [];
  }

  protected formTitle(cfg: ResourceConfig): string {
    return this.formMode() === 'edit'
      ? `${this.i18n.auto('Modifier')} · ${this.i18n.auto(cfg.title)}`
      : this.createLabel(cfg);
  }

  protected canOpenForm(cfg: ResourceConfig): boolean {
    return this.formMode() === 'edit' ? this.canMutate(cfg) : this.canCreate(cfg);
  }

  protected hasEditableFields(cfg: ResourceConfig): boolean {
    return Boolean(cfg.editable) && Boolean((cfg.patchFields ?? cfg.createFields)?.length);
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
          this.loadAvatars(cfg);
        },
        error: (err: unknown) => this.error.set(describeHttpError(err, 'Chargement')),
      });
  }

  protected submit(cfg: ResourceConfig): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }
    const fields = this.effectiveFields(cfg);
    const payload = this.formPayload(fields);
    const editing = this.formMode() === 'edit';
    this.saving.set(true);
    this.error.set(null);
    const request = editing
      ? this.api.patch<Row>(`${cfg.endpoint}/${this.editingId()}`, payload)
      : this.api.post<Row>(cfg.endpoint, payload);
    request.subscribe({
      next: () => {
        this.saving.set(false);
        this.closeForm();
        this.message.set(editing ? 'Modifications enregistrees.' : 'Enregistrement effectue.');
        this.buildForm(cfg.createFields ?? []);
        this.load();
      },
      error: (err: unknown) => {
        this.saving.set(false);
        this.error.set(describeHttpError(err, 'Enregistrement'));
      },
    });
  }

  protected importAgents(): void {
    if (!this.importCommuneId.trim() || !this.importCsv.trim()) {
      this.importResult.set('Commune et contenu CSV sont requis.');
      return;
    }
    this.api
      .postText<{
        created: number;
        updated: number;
        skipped: number;
      }>('/api/v1/agents/import-csv', this.importCsv, { commune_id: this.importCommuneId.trim() })
      .subscribe({
        next: (result) => {
          this.showImportDialog.set(false);
          this.message.set(
            `${result.created} cree(s), ${result.updated} mis a jour, ${result.skipped} ignore(s).`,
          );
          this.load();
        },
        error: (err: unknown) => this.importResult.set(describeHttpError(err, 'Import')),
      });
  }

  protected runAction(action: ResourceAction, row: Row, event?: MouseEvent): void {
    event?.stopPropagation();
    this.rowMenuKey.set(null);
    if (action.kind === 'download') {
      this.api.openDownload(action.path(row), action.filename?.(row) ?? 'document', undefined, (err) =>
        this.error.set(describeHttpError(err, 'Telechargement')),
      );
      return;
    }
    if (action.kind === 'share') {
      void this.runShare(action, row);
      return;
    }
    if (action.kind === 'status') {
      this.openStatus(action, row);
      return;
    }
    if (action.sensitive || action.kind === 'delete') {
      this.pendingAction.set({ action, row });
      return;
    }
    this.executeAction(action, row);
  }

  /**
   * Partage natif du PV (Web Share API) : lien de suivi + PDF si le device le permet.
   * En attendant l'API WhatsApp, l'utilisateur choisit le destinataire via le sélecteur
   * natif. Fallback desktop : copie du lien dans le presse-papier.
   */
  private async runShare(action: ResourceAction, row: Row): Promise<void> {
    const url = action.shareUrl?.(row) ?? '';
    const text = action.shareText?.(row) ?? '';
    const title = String(row['pv_number'] ?? action.label);

    let file: File | undefined;
    if (action.path && action.filename) {
      try {
        const blob = await firstValueFrom(this.api.download(action.path(row)));
        file = new File([blob], action.filename(row), {
          type: blob.type || 'application/pdf',
        });
      } catch {
        file = undefined;
      }
    }

    if (typeof navigator.share === 'function') {
      const canShareFile = !!file && navigator.canShare?.({ files: [file] });
      const data: ShareData = canShareFile ? { title, text, url, files: [file!] } : { title, text, url };
      try {
        await navigator.share(data);
      } catch {
        // Partage annulé par l'utilisateur : rien à signaler.
      }
      return;
    }

    try {
      await navigator.clipboard?.writeText(url || text);
      this.message.set('Lien du PV copié dans le presse-papier.');
    } catch {
      this.message.set("Partage non supporté sur cet appareil.");
    }
  }

  protected visibleActions(cfg: ResourceConfig): ResourceAction[] {
    return (cfg.actions ?? []).filter((action) => this.actionAllowed(cfg, action));
  }

  private actionAllowed(cfg: ResourceConfig, action: ResourceAction): boolean {
    if (action.roles) {
      return this.auth.hasAnyRole(action.roles);
    }
    if (action.kind === 'download' || action.kind === 'share') {
      return true;
    }
    return this.canMutate(cfg);
  }

  private openStatus(action: ResourceAction, row: Row): void {
    const current = String(row[action.statusFromKey ?? 'status'] ?? '');
    const options = this.allowedStatusOptions(action, current);
    const controls: Record<string, FormControl<unknown>> = {
      status: new FormControl('', [Validators.required]),
    };
    for (const extra of action.statusExtra ?? []) {
      controls[extra.key] = new FormControl('', extra.required ? [Validators.required] : []);
    }
    // Relations contextuelles : recharge les options avec les paramètres dérivés
    // de la ligne (ex. « Affecter à » limité à la commune du signalement).
    const rowRelations = new Map<string, RelationConfig>();
    for (const extra of action.statusExtra ?? []) {
      if (extra.relation && extra.rowQuery) {
        rowRelations.set(extra.key, {
          ...extra.relation,
          query: { ...(extra.relation.query ?? {}), ...extra.rowQuery(row) },
        });
      }
    }
    if (rowRelations.size > 0) {
      this.lookup.clear([...rowRelations.keys()]);
      this.lookup.loadRelations(rowRelations);
    }
    this.statusForm = new FormGroup(controls);
    this.statusContext.set({ action, row, options, current });
  }

  private allowedStatusOptions(action: ResourceAction, current: string): SelectOption[] {
    const all = action.statusOptions ?? [];
    if (action.statusTransitions) {
      const allowed = action.statusTransitions[current] ?? [];
      return all.filter((opt) => allowed.includes(String(opt.value)));
    }
    return all.filter((opt) => String(opt.value) !== current);
  }

  protected submitStatus(ctx: StatusContext): void {
    if (this.statusForm.invalid) {
      this.statusForm.markAllAsTouched();
      return;
    }
    const raw = this.statusForm.getRawValue() as Record<string, unknown>;
    const payload: Record<string, unknown> = { [ctx.action.statusKey ?? 'status']: raw['status'] };
    for (const extra of ctx.action.statusExtra ?? []) {
      const value = raw[extra.key];
      if (value !== '' && value !== null && value !== undefined) {
        payload[extra.key] = value;
      }
    }
    this.saving.set(true);
    this.error.set(null);
    const path = ctx.action.path(ctx.row);
    const request$ =
      ctx.action.method === 'post' ? this.api.post(path, payload) : this.api.patch(path, payload);
    const successMessage = ctx.action.successMessage ?? 'Statut mis a jour.';
    request$.subscribe({
      next: () => {
        this.saving.set(false);
        this.statusContext.set(null);
        this.message.set(successMessage);
        this.load();
      },
      error: (err: unknown) => {
        this.saving.set(false);
        this.statusContext.set(null);
        this.error.set(describeHttpError(err, 'Changement de statut'));
      },
    });
  }

  protected confirmPendingAction(): void {
    const pending = this.pendingAction();
    if (!pending) {
      return;
    }
    this.pendingAction.set(null);
    this.executeAction(pending.action, pending.row);
  }

  /** Clic sur une ligne : aperçu rapide en drawer. Le titre (colonne 1) ouvre la fiche complète. */
  protected selectRow(row: Row): void {
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
    if (this.agentsDialog()) {
      this.agentsDialog.set(null);
    } else if (this.statusContext()) {
      this.statusContext.set(null);
    } else if (this.pendingAction()) {
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
      return this.i18n.auto('Nouveau PV');
    }
    if (cfg.key === 'referentiel-interventions') {
      return this.i18n.auto('Nouvelle intervention');
    }
    return this.i18n.auto('Nouvel element');
  }

  protected formSections(fields: ResourceField[]): FormSection[] {
    const sections = new Map<string, ResourceField[]>();
    for (const field of fields) {
      if (!this.isFieldVisible(field)) {
        continue;
      }
      const title = field.section ?? 'Informations';
      sections.set(title, [...(sections.get(title) ?? []), field]);
    }
    return Array.from(sections.entries()).map(([title, sectionFields]) => ({
      title,
      fields: sectionFields,
    }));
  }

  protected optionsFor(key: string): LookupOption[] {
    return this.lookup.optionsFor(key);
  }

  protected optionsForField(field: ResourceField): LookupOption[] {
    const options = this.optionsFor(field.key);
    if (!field.dependsOn) {
      return options;
    }
    const parentValue = this.form.get(field.dependsOn)?.value;
    if (parentValue === null || parentValue === undefined || parentValue === '') {
      return [];
    }
    return options.filter((option) => !option.parentId || option.parentId === String(parentValue));
  }

  protected optionLabel(option: LookupOption): string {
    return option.meta ? `${option.label} - ${option.meta}` : option.label;
  }

  protected selectedInterventionMeta(): string {
    const selected = this.selectedInterventionIds();
    if (!selected.length) {
      return 'Choisis une ou plusieurs infractions pour afficher le montant officiel connu du referentiel.';
    }
    const options = selected
      .map(
        (id) =>
          this.optionsFor('intervention_ids').find((item) => item.id === id) ??
          this.optionsFor('intervention_id').find((item) => item.id === id),
      )
      .filter((item): item is LookupOption => Boolean(item));
    if (!options.length) {
      return `${selected.length} infraction(s) selectionnee(s). Le backend calculera le montant officiel.`;
    }
    const total = options.reduce((sum, option) => sum + Number(option.meta ?? 0), 0);
    const labels = options.map((option) => option.label).join(', ');
    return total > 0
      ? `${labels} - total connu ${total.toLocaleString('fr-FR')} FCFA`
      : `${labels} - montant calcule par le backend.`;
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
    return this.i18n.auto(cfg.labels[column] ?? column);
  }

  protected avatarFor(row: Row): string | null {
    const id = row['id'];
    return id ? (this.avatarUrls()[String(id)] ?? null) : null;
  }

  protected initials(title: string): string {
    return title
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((part) => part.charAt(0).toUpperCase())
      .join('');
  }

  /** Récupère les avatars (blob authentifié → object URL) des lignes qui en possèdent. */
  private loadAvatars(cfg: ResourceConfig): void {
    if (!cfg.photoEndpoint) {
      return;
    }
    const endpoint = cfg.photoEndpoint;
    for (const row of this.rows()) {
      const id = row['id'];
      if (!id || !row['has_photo']) {
        continue;
      }
      const key = String(id);
      if (this.avatarUrls()[key]) {
        continue;
      }
      this.api.download(endpoint(key)).subscribe({
        next: (blob) =>
          this.avatarUrls.update((current) => ({ ...current, [key]: URL.createObjectURL(blob) })),
        error: () => {},
      });
    }
  }

  private clearAvatars(): void {
    for (const url of Object.values(this.avatarUrls())) {
      URL.revokeObjectURL(url);
    }
    this.avatarUrls.set({});
  }

  protected display(column: string, value: unknown): string {
    if (value === null || value === undefined || value === '') {
      return '-';
    }
    if (column === 'subject_type') {
      return this.displaySubjectType(value);
    }
    if (column === 'interventions') {
      return this.displayInterventions(value);
    }
    const relation = this.lookup.label(column, value);
    if (relation) {
      return relation;
    }
    if (Array.isArray(value)) {
      return value.join(', ');
    }
    if (typeof value === 'boolean') {
      return this.i18n.yesNo(value);
    }
    if (this.isMoneyColumn(column)) {
      return this.i18n.formatMoneyFcfa(value ?? 0);
    }
    if (typeof value === 'string' && value.includes('T') && value.endsWith('Z')) {
      return this.i18n.formatDate(value);
    }
    if (column === 'status') {
      return this.i18n.auto(this.humanStatus(String(value)));
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
    if (field.type === 'datetime') {
      return 'datetime-local';
    }
    return field.type;
  }

  protected applyTableState(): void {
    const cfg = this.config();
    const query = this.search.trim().toLowerCase();
    let rows = this.rows().filter((row) => this.matchesLocalFilters(cfg, row));

    if (query) {
      rows = rows.filter((row) => this.searchableText(row).toLowerCase().includes(query));
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
    const header = columns.map((column) => this.label(cfg, column));
    const lines = rows.map((row) => columns.map((column) => this.display(column, row[column])));
    saveCsv(filename, [header, ...lines]);
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
      error: (err: unknown) => this.error.set(describeHttpError(err, 'Action')),
    });
  }

  private buildForm(fields: ResourceField[]): void {
    const controls: Record<string, FormControl<unknown>> = {};
    for (const field of fields) {
      if (field.type === 'geopoint') {
        controls[field.latKey ?? field.key] = new FormControl(null);
        controls[field.lonKey ?? 'gps_longitude'] = new FormControl(null);
        continue;
      }
      if (field.type === 'geopolygon') {
        controls[field.key] = new FormControl(null);
        continue;
      }
      if (field.type === 'relation_multi' || field.type === 'select_multi') {
        controls[field.key] = new FormControl([], field.required ? [Validators.required] : []);
        continue;
      }
      controls[field.key] = new FormControl(
        this.defaultValue(field),
        field.required ? [Validators.required] : [],
      );
    }
    this.form = new FormGroup(controls);
    this.formFields = fields;
    this.conditionalSub?.unsubscribe();
    // Réévalue l'affichage conditionnel (visibleWhen) à chaque changement de valeur.
    this.conditionalSub = this.form.valueChanges.subscribe(() => this.applyConditionalState());
    this.applyConditionalState();
  }

  /**
   * Un champ `visibleWhen` est masqué (et son contrôle désactivé) tant que le champ
   * pilote n'a pas la valeur attendue. Désactiver le contrôle l'exclut de la validation
   * Angular et du payload — indispensable pour ne pas bloquer la soumission sur un champ
   * requis mais caché (ex. « Raison sociale » quand la personne est physique).
   */
  private applyConditionalState(): void {
    for (const field of this.formFields) {
      if (!field.visibleWhen) {
        continue;
      }
      const control = this.form.get(field.key);
      if (!control) {
        continue;
      }
      const visible = this.matchesVisibleWhen(field.visibleWhen);
      if (visible && control.disabled) {
        control.enable({ emitEvent: false });
      } else if (!visible && control.enabled) {
        control.disable({ emitEvent: false });
      }
    }
  }

  private matchesVisibleWhen(rule: { field: string; equals: string | string[] }): boolean {
    const current = String(this.form.get(rule.field)?.value ?? '');
    const expected = Array.isArray(rule.equals) ? rule.equals : [rule.equals];
    return expected.includes(current);
  }

  protected isFieldVisible(field: ResourceField): boolean {
    return !field.visibleWhen || this.matchesVisibleWhen(field.visibleWhen);
  }

  private patchForm(fields: ResourceField[], row: Row): void {
    for (const field of fields) {
      if (field.type === 'geopoint') {
        const latKey = field.latKey ?? field.key;
        const lonKey = field.lonKey ?? 'gps_longitude';
        this.form.controls[latKey]?.setValue(this.numOrNull(row[latKey]));
        this.form.controls[lonKey]?.setValue(this.numOrNull(row[lonKey]));
        continue;
      }
      if (field.type === 'geopolygon') {
        this.form.controls[field.key]?.setValue(row[field.key] ?? null);
        continue;
      }
      const control = this.form.controls[field.key];
      if (!control) {
        continue;
      }
      const raw = row[field.key];
      if (field.type === 'checkbox') {
        control.setValue(Boolean(raw));
      } else if (field.type === 'relation_multi') {
        control.setValue(this.relationMultiValue(field.key, row));
      } else if (field.type === 'select_multi') {
        control.setValue(
          Array.isArray(raw) ? raw.map((item) => String(item)) : raw ? [String(raw)] : [],
        );
      } else if (raw === null || raw === undefined) {
        control.setValue('');
      } else if (field.type === 'relation') {
        control.setValue(String(raw));
      } else if (field.type === 'datetime') {
        control.setValue(this.toDatetimeLocal(raw));
      } else {
        control.setValue(raw);
      }
    }
    this.backfillCascadeParents();
    this.applyConditionalState();
  }

  /**
   * En édition, un champ en cascade (ex. `commune_id` filtré par `departement_id`) doit
   * retrouver la valeur de ses parents pour rester affichable. On dérive la valeur parente
   * depuis le `parentId` de l'option enfant chargée (commune → département → région).
   * Parcours en ordre inverse pour propager en une passe.
   */
  private backfillCascadeParents(): void {
    for (const field of [...this.formFields].reverse()) {
      if (!field.dependsOn) {
        continue;
      }
      const control = this.form.get(field.key);
      const parentControl = this.form.get(field.dependsOn);
      if (!control || !parentControl) {
        continue;
      }
      const childValue = control.value;
      if (!childValue || parentControl.value) {
        continue;
      }
      const option = this.optionsFor(field.key).find((item) => item.id === String(childValue));
      if (option?.parentId) {
        parentControl.setValue(option.parentId);
      }
    }
  }

  private defaultValue(field: ResourceField): unknown {
    if (field.default !== undefined) {
      return field.default;
    }
    if (field.type === 'checkbox') {
      return field.key === 'active' ? true : false;
    }
    if (field.type === 'relation_multi') {
      return [];
    }
    return '';
  }

  private numOrNull(value: unknown): number | null {
    return value === '' || value === null || value === undefined ? null : Number(value);
  }

  /** Convertit une date ISO de l'API vers la valeur locale « YYYY-MM-DDTHH:mm »
   *  attendue par <input type="datetime-local">. */
  private toDatetimeLocal(value: unknown): string {
    const parsed = new Date(String(value));
    if (Number.isNaN(parsed.getTime())) {
      return '';
    }
    const pad = (n: number) => String(n).padStart(2, '0');
    return (
      `${parsed.getFullYear()}-${pad(parsed.getMonth() + 1)}-${pad(parsed.getDate())}` +
      `T${pad(parsed.getHours())}:${pad(parsed.getMinutes())}`
    );
  }

  /** Champs occupant toute la largeur du formulaire (zone d'édition large). */
  protected isWideField(field: ResourceField): boolean {
    return field.type === 'textarea' || field.type === 'geopoint' || field.type === 'geopolygon';
  }

  protected geoNumber(key: string): number | null {
    return this.numOrNull(this.form.get(key)?.value);
  }

  protected geoBoundary(key: string): GeoGeometry | null {
    const value = this.form.get(key)?.value;
    return (value ?? null) as GeoGeometry | null;
  }

  protected setControl(key: string, value: unknown): void {
    const control = this.form.get(key);
    if (control) {
      control.setValue(value);
      control.markAsDirty();
    }
  }

  /** Pré-remplit le champ "Lieu" d'un PV avec l'adresse géocodée s'il est vide. */
  protected onAddressResolved(address: string): void {
    const control = this.form.get('location_description');
    if (control && !String(control.value ?? '').trim()) {
      control.setValue(address);
    }
  }

  private formPayload(fields: ResourceField[]): Record<string, unknown> {
    const raw = this.form.getRawValue() as Record<string, unknown>;
    const payload: Record<string, unknown> = {};
    for (const field of fields) {
      // Champs purement UI (filtres Région/Département) ou masqués : jamais envoyés.
      if (field.uiOnly || !this.isFieldVisible(field)) {
        continue;
      }
      if (field.type === 'geopoint') {
        const latKey = field.latKey ?? field.key;
        const lonKey = field.lonKey ?? 'gps_longitude';
        payload[latKey] = this.numOrNull(raw[latKey]);
        payload[lonKey] = this.numOrNull(raw[lonKey]);
        continue;
      }
      if (field.type === 'geopolygon') {
        const boundary = raw[field.key];
        if (boundary && typeof boundary === 'object') {
          payload[field.key] = boundary;
        }
        continue;
      }
      const value = raw[field.key];
      if (field.type === 'checkbox') {
        payload[field.key] = Boolean(value);
      } else if (field.type === 'datetime') {
        // <input datetime-local> renvoie « YYYY-MM-DDTHH:mm » (heure locale, sans
        // fuseau). L'API attend du RFC 3339 → on convertit en ISO 8601 UTC.
        const text = String(value ?? '').trim();
        if (text) {
          const parsed = new Date(text);
          payload[field.key] = Number.isNaN(parsed.getTime()) ? text : parsed.toISOString();
        }
      } else if (field.type === 'number' || field.type === 'money') {
        payload[field.key] = value === '' || value === null ? null : Number(value);
      } else if (field.type === 'relation_multi' || field.type === 'select_multi') {
        const values = Array.isArray(value)
          ? value.map((item) => String(item)).filter(Boolean)
          : String(value ?? '')
              .split(',')
              .map((item) => item.trim())
              .filter(Boolean);
        if (values.length) {
          payload[field.key] = values;
          if (field.key === 'intervention_ids') {
            payload['intervention_id'] = values[0];
          }
        }
      } else if (field.type === 'array') {
        const values = String(value ?? '')
          .split(',')
          .map((item) => item.trim())
          .filter(Boolean);
        if (values.length) {
          payload[field.key] = values;
        }
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
    this.clearAvatars();
    this.lookup.reset();
    this.selectedRow.set(null);
    this.pendingAction.set(null);
    this.statusContext.set(null);
    this.agentsDialog.set(null);
    this.formMode.set('create');
    this.editingId.set(null);
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

  private selectedInterventionIds(): string[] {
    const multi = this.form.controls['intervention_ids']?.value;
    if (Array.isArray(multi)) {
      return multi.map((item) => String(item)).filter(Boolean);
    }
    const legacy = this.form.controls['intervention_id']?.value;
    return legacy ? [String(legacy)] : [];
  }

  private relationMultiValue(key: string, row: Row): string[] {
    const raw = row[key];
    if (Array.isArray(raw)) {
      return raw.map((item) => String(item)).filter(Boolean);
    }
    if (key === 'intervention_ids' && Array.isArray(row['interventions'])) {
      return row['interventions']
        .map((item) => (this.isRecord(item) ? item['intervention_id'] : null))
        .filter((item): item is string => typeof item === 'string' && item.length > 0);
    }
    return [];
  }

  private displaySubjectType(value: unknown): string {
    switch (String(value)) {
      case 'PERSON_ONLY':
        return 'Usager sans vehicule';
      case 'VEHICLE_ONLY':
        return 'Vehicule sans conducteur';
      case 'PERSON_WITH_VEHICLE':
        return 'Usager avec vehicule';
      default:
        return String(value);
    }
  }

  private displayInterventions(value: unknown): string {
    if (!Array.isArray(value)) {
      return this.display('intervention_id', value);
    }
    const labels = value
      .map((item) => {
        if (!this.isRecord(item)) {
          return String(item);
        }
        const name = String(item['nom'] ?? item['intervention_id'] ?? '').trim();
        const amount = Number(item['montant_fcfa'] ?? 0);
        return amount > 0 ? `${name} (${amount.toLocaleString('fr-FR')} FCFA)` : name;
      })
      .filter(Boolean);
    return labels.length ? labels.join(', ') : '-';
  }

  private isRecord(value: unknown): value is Row {
    return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
  }

  private canUse(roles: RoleCode[] | undefined): boolean {
    return !roles || this.auth.hasAnyRole(roles);
  }
}
