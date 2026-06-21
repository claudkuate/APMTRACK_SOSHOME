import { Injectable, inject, signal } from '@angular/core';

import { LookupOption } from '../../shared/api-types';
import { RelationConfig, ResourceConfig } from '../../shared/resource-config';
import { ApiService } from './api.service';

type Row = Record<string, unknown>;
type LookupState = Record<string, LookupOption[]>;

/**
 * Résolution centralisée « id → libellé métier ».
 *
 * Charge les options de chaque relation déclarée sur une config (formulaires, filtres,
 * actions, `displayRelations`) et expose un accès par clé de champ. Partagé par la liste
 * (`ResourcePage`) et le détail (`ResourceDetailPage`) pour garantir des libellés cohérents
 * et ne jamais afficher d'UUID brut.
 */
@Injectable({ providedIn: 'root' })
export class LookupService {
  private readonly api = inject(ApiService);
  private readonly state = signal<LookupState>({});

  /** Signal lecture seule (utile pour réagir au chargement des options dans un template). */
  readonly lookups = this.state.asReadonly();

  reset(): void {
    this.state.set({});
  }

  optionsFor(key: string): LookupOption[] {
    return this.state()[key] ?? [];
  }

  /** Libellé métier d'un id pour un champ donné, ou `null` si non résolvable. */
  label(field: string, value: unknown): string | null {
    if (value === null || value === undefined || value === '') {
      return null;
    }
    const id = String(value);
    return this.optionsFor(field).find((item) => item.id === id)?.label ?? null;
  }

  /** Charge toutes les relations résolvables d'une config (idempotent par clé). */
  loadForConfig(cfg: ResourceConfig): void {
    this.loadRelations(relationsForConfig(cfg));
  }

  loadRelations(relations: Map<string, RelationConfig>): void {
    for (const [key, relation] of relations) {
      this.api
        .page<Row>(relation.endpoint, { page_size: 100, ...(relation.query ?? {}) })
        .subscribe({
          next: (response) => {
            const options = response.items.map((row) => toOption(row, relation));
            this.state.update((current) => ({ ...current, [key]: options }));
          },
          error: () => {
            this.state.update((current) => ({ ...current, [key]: current[key] ?? [] }));
          },
        });
    }
  }
}

/** Agrège toutes les relations d'une config, indexées par clé de champ. */
export function relationsForConfig(cfg: ResourceConfig): Map<string, RelationConfig> {
  const relations = new Map<string, RelationConfig>();
  for (const field of cfg.createFields ?? []) {
    if (field.relation) {
      relations.set(field.key, field.relation);
    }
  }
  for (const field of cfg.patchFields ?? []) {
    if (field.relation) {
      relations.set(field.key, field.relation);
    }
  }
  for (const filter of cfg.filters ?? []) {
    if (filter.relation) {
      relations.set(filter.key, filter.relation);
    }
  }
  for (const action of cfg.actions ?? []) {
    for (const extra of action.statusExtra ?? []) {
      if (extra.relation) {
        relations.set(extra.key, extra.relation);
      }
    }
  }
  for (const [key, relation] of Object.entries(cfg.displayRelations ?? {})) {
    relations.set(key, relation);
  }
  return relations;
}

function toOption(row: Row, relation: RelationConfig): LookupOption {
  const valueKey = relation.valueKey ?? 'id';
  const id = String(row[valueKey] ?? '');
  return {
    id,
    label: String(row[relation.labelKey] ?? id),
    meta:
      row[relation.metaKey ?? ''] === undefined ? undefined : String(row[relation.metaKey ?? '']),
    status:
      row[relation.statusKey ?? ''] === undefined
        ? undefined
        : String(row[relation.statusKey ?? '']),
    parentId:
      row[relation.parentKey ?? ''] === undefined
        ? undefined
        : String(row[relation.parentKey ?? '']),
  };
}
