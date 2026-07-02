import { RoleCode } from './api-types';
import { apiBaseUrl } from '../core/config/runtime-config';

export type FieldType =
  | 'text'
  | 'email'
  | 'password'
  | 'number'
  | 'money'
  | 'date'
  | 'checkbox'
  | 'textarea'
  | 'array'
  | 'datetime'
  | 'select'
  | 'select_multi'
  | 'relation'
  | 'relation_multi'
  | 'status'
  | 'geopoint'
  | 'geopolygon';

export type FilterType = 'search' | 'status' | 'active' | 'relation' | 'dateRange';

export interface SelectOption {
  value: string | number | boolean;
  label: string;
}

export interface RelationConfig {
  endpoint: string;
  valueKey?: string;
  labelKey: string;
  metaKey?: string;
  statusKey?: string;
  parentKey?: string;
  query?: Record<string, string | number | boolean>;
}

export interface ResourceField {
  key: string;
  label: string;
  type: FieldType;
  required?: boolean;
  placeholder?: string;
  help?: string;
  section?: string;
  options?: SelectOption[];
  relation?: RelationConfig;
  readonly?: boolean;
  dependsOn?: string;
  /** Champ purement UI (ex. filtre Région/Département) : jamais envoyé dans le payload. */
  uiOnly?: boolean;
  /** Affiche ce champ uniquement quand le champ `field` vaut `equals` (valeur ou liste). */
  visibleWhen?: { field: string; equals: string | string[] };
  /** Valeur initiale du contrôle à la création (sinon vide / false / []). */
  default?: string | number | boolean;
  /** geopoint: clés de formulaire alimentées par le sélecteur de position. */
  latKey?: string;
  lonKey?: string;
}

export interface ResourceFilter {
  key: string;
  label: string;
  type: FilterType;
  queryKey?: string;
  options?: SelectOption[];
  relation?: RelationConfig;
}

export interface StatusExtraField {
  key: string;
  label: string;
  type: 'textarea' | 'text' | 'relation';
  relation?: RelationConfig;
  required?: boolean;
  placeholder?: string;
  /** Params de requête dérivés de la ligne : les options de la relation sont
   *  rechargées à l'ouverture du dialogue avec ces paramètres (fusionnés à
   *  `relation.query`). Ex. restreindre « Affecter à » à la commune de la ligne. */
  rowQuery?: (row: Record<string, unknown>) => Record<string, string | number | boolean>;
}

export interface ResourceAction {
  label: string;
  kind: 'download' | 'post' | 'delete' | 'status' | 'share';
  path: (row: Record<string, unknown>) => string;
  filename?: (row: Record<string, unknown>) => string;
  /** share-kind: lien de suivi à partager (ouvre le partage natif du device). */
  shareUrl?: (row: Record<string, unknown>) => string;
  /** share-kind: texte accompagnant le partage. */
  shareText?: (row: Record<string, unknown>) => string;
  sensitive?: boolean;
  confirmTitle?: string;
  confirmMessage?: (row: Record<string, unknown>) => string;
  /** Restrict action visibility to these roles. Defaults to the resource mutateRoles. */
  roles?: RoleCode[];
  // status-kind specifics ------------------------------------------------------
  /** Candidate target statuses for a 'status' action. */
  statusOptions?: SelectOption[];
  /** Row key holding the current status (default 'status'). */
  statusFromKey?: string;
  /** Allowed transitions keyed by current status. When omitted, all statusOptions
   *  except the current value are offered. */
  statusTransitions?: Record<string, string[]>;
  /** Extra fields submitted alongside the status (reason, notes, assignment...). */
  statusExtra?: StatusExtraField[];
  /** Body key carrying the selected value (default 'status'). Use 'target' for escalation. */
  statusKey?: string;
  /** HTTP method for a 'status' action (default 'patch'). */
  method?: 'patch' | 'post';
  /** Override the select label in the dialog (default 'Nouveau statut'). */
  selectLabel?: string;
  /** Override the "current value" caption in the dialog (default 'Statut courant'). */
  currentLabel?: string;
  /** Override the success toast (default 'Statut mis a jour.'). */
  successMessage?: string;
}

/** Section « entités liées » affichée sous le détail d'une fiche. */
export interface RelatedSection {
  /** Clé de la ressource enfant dans `resourceConfigs`. */
  key: string;
  title: string;
  /** Champ de l'enfant pointant vers l'id de la fiche courante (ex. `commune_id`). */
  foreignKey: string;
  /** Colonnes de l'enfant à afficher (défaut: colonnes principales hors `*_id`). */
  columns?: string[];
}

/** Configuration de la page détail dédiée d'une ressource. */
export interface ResourceDetailConfig {
  /** Champs mis en avant dans le bandeau d'en-tête (défaut: premiers `detailFields`). */
  summaryFields?: string[];
  /** Sous-ressources listées en sections sous le détail. */
  related?: RelatedSection[];
}

export interface ResourceConfig {
  key: string;
  title: string;
  description: string;
  endpoint: string;
  columns: string[];
  secondaryColumns?: string[];
  detailFields?: string[];
  /** Relations utilisées pour résoudre un id en libellé à l'affichage (hors formulaire). */
  displayRelations?: Record<string, RelationConfig>;
  /** Structure de la page détail dédiée (en-tête, sections liées). */
  detail?: ResourceDetailConfig;
  labels: Record<string, string>;
  createFields?: ResourceField[];
  patchFields?: ResourceField[];
  createRoles?: RoleCode[];
  mutateRoles?: RoleCode[];
  /** Enables in-place edit (PATCH {endpoint}/{id}). Only set where the API exposes a
   *  generic PATCH endpoint for the resource. */
  editable?: boolean;
  query?: Record<string, string | number | boolean>;
  filters?: ResourceFilter[];
  actions?: ResourceAction[];
  /** Enables the dedicated "Gérer les agents" dialog (patrouilles sub-resource). */
  manageAgents?: boolean;
  /** Active le bloc photo de profil sur la fiche détail (upload + aperçu).
   *  Renvoie l'endpoint API (GET pour servir, POST multipart pour téléverser). */
  photoEndpoint?: (id: string) => string;
}

const statusOptions: Record<string, SelectOption[]> = {
  agents: [
    option('ACTIF', 'Actif'),
    option('SUSPENDU', 'Suspendu'),
    option('RETRAITE', 'Retraite'),
  ],
  pvs: [
    option('EN_ATTENTE_PAIEMENT', 'En attente paiement'),
    option('PAYE', 'Payé'),
    option('EN_RETARD', 'En retard'),
    option('NON_PAYANT', 'Non payant'),
    option('ANNULE', 'Annulé'),
    option('CONTESTE', 'Contesté'),
  ],
  signalements: [
    option('RECU', 'Reçu'),
    option('EN_COURS', 'En cours'),
    option('TRAITE', 'Traité'),
    option('CLASSE', 'Classé'),
    option('REJETE', 'Rejeté'),
  ],
  patrouilles: [
    option('PLANIFIEE', 'Planifiée'),
    option('EN_COURS', 'En cours'),
    option('CLOTUREE', 'Clôturée'),
    option('ANNULEE', 'Annulée'),
  ],
  fourrieres: [
    option('EN_FOURRIERE', 'En fourrière'),
    option('RESTITUE', 'Restitué'),
    option('VENDU', 'Vendu'),
    option('DETRUIT', 'Détruit'),
  ],
};

const pvSubjectTypeOptions: SelectOption[] = [
  option('PERSON_WITH_VEHICLE', 'Usager avec véhicule'),
  option('PERSON_ONLY', 'Usager sans véhicule'),
];

const communeRelation: RelationConfig = {
  endpoint: '/api/v1/communes',
  labelKey: 'nom',
  metaKey: 'code',
  statusKey: 'active',
  // Permet la cascade Région → Département → Commune (filtrage client par département).
  // Sans effet sur les usages plats (filtres, displayRelations) qui ignorent parentId.
  parentKey: 'departement_id',
};

const regionRelation: RelationConfig = {
  endpoint: '/api/v1/geography/regions',
  labelKey: 'nom',
  metaKey: 'code',
};

const departementRelation: RelationConfig = {
  endpoint: '/api/v1/geography/departements',
  labelKey: 'nom',
  parentKey: 'region_id',
};

const zoneRelation: RelationConfig = {
  endpoint: '/api/v1/zones',
  labelKey: 'nom',
  metaKey: 'type_zone',
  parentKey: 'commune_id',
  statusKey: 'active',
};

const categoryRelation: RelationConfig = {
  endpoint: '/api/v1/referentiel/categories',
  labelKey: 'nom',
  metaKey: 'description',
  parentKey: 'commune_id',
  statusKey: 'active',
};

const typeRelation: RelationConfig = {
  endpoint: '/api/v1/referentiel/types',
  labelKey: 'nom',
  metaKey: 'description',
  parentKey: 'category_id',
  statusKey: 'active',
};

const interventionRelation: RelationConfig = {
  endpoint: '/api/v1/referentiel/interventions',
  labelKey: 'nom',
  metaKey: 'montant_fcfa',
  parentKey: 'type_id',
  statusKey: 'active',
};

const userRelation: RelationConfig = {
  endpoint: '/api/v1/users',
  labelKey: 'full_name',
  metaKey: 'email',
  statusKey: 'active',
};

const agentRelation: RelationConfig = {
  endpoint: '/api/v1/agents',
  labelKey: 'full_name',
  metaKey: 'matricule',
  parentKey: 'commune_id',
  statusKey: 'status',
};

/** Agents actifs uniquement — pour les sélecteurs d'affectation (effectif patrouille). */
const activeAgentRelation: RelationConfig = {
  ...agentRelation,
  query: { status: 'ACTIF' },
};

const pvRelation: RelationConfig = {
  endpoint: '/api/v1/pvs',
  labelKey: 'pv_number',
  metaKey: 'verbalized_name',
  parentKey: 'commune_id',
  statusKey: 'status',
};

/** Détecte une valeur ressemblant à un UUID afin de ne jamais l'afficher en clair. */
export function isUuidLike(value: unknown): boolean {
  return (
    typeof value === 'string' &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value)
  );
}

/** Mappe un `entity_type`/table d'audit vers la feature (route détail) correspondante. */
export function featureForEntityType(type: unknown): string | null {
  const map: Record<string, string> = {
    communes: 'communes',
    commune: 'communes',
    users: 'users',
    user: 'users',
    agents: 'agents',
    agent: 'agents',
    zones: 'zones',
    zone: 'zones',
    pvs: 'pvs',
    pv: 'pvs',
    signalements: 'signalements',
    signalement: 'signalements',
    patrouilles: 'patrouilles',
    patrouille: 'patrouilles',
    intervention_types: 'referentiel-types',
    intervention_categories: 'referentiel-categories',
    interventions: 'referentiel-interventions',
  };
  return map[String(type ?? '').toLowerCase()] ?? null;
}

export const resourceConfigs: Record<string, ResourceConfig> = {
  communes: {
    key: 'communes',
    title: 'Communes',
    description: 'Paramétrage institutionnel et périmètres de travail.',
    endpoint: '/api/v1/communes',
    editable: true,
    columns: ['code', 'nom', 'region', 'departement', 'active', 'subscription_status'],
    secondaryColumns: ['telephone', 'email'],
    detailFields: [
      'code',
      'nom',
      'region',
      'departement',
      'adresse',
      'telephone',
      'email',
      'theme_color',
      'active',
      'subscription_status',
      'subscription_started_at',
      'subscription_expires_at',
      'subscription_active',
      'public_visible',
    ],
    detail: {
      summaryFields: ['code', 'region', 'departement'],
      related: [
        { key: 'zones', title: 'Zones', foreignKey: 'commune_id' },
        { key: 'agents', title: 'Agents', foreignKey: 'commune_id' },
        { key: 'pvs', title: 'PV récents', foreignKey: 'commune_id' },
      ],
    },
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [activeFilter()],
    createFields: [
      field('code', 'Code commune', 'text', true, 'YDE1'),
      field('nom', 'Nom officiel', 'text', true),
      relationField('region_id', 'Région', regionRelation, true),
      relationField('departement_id', 'Département', departementRelation, true, undefined, 'region_id'),
      field('adresse', 'Adresse', 'text'),
      field('telephone', 'Téléphone', 'text'),
      field('email', 'Email', 'email'),
      field('theme_color', 'Couleur thème', 'text', false, '#1F7A4D'),
      field('active', 'Commune active', 'checkbox'),
      selectField(
        'subscription_status',
        'Statut abonnement',
        false,
        [
          option('ACTIVE', 'Actif'),
          option('TRIAL', 'Essai'),
          option('EXPIRED', 'Expiré'),
          option('SUSPENDED', 'Suspendu'),
        ],
      ),
      field('subscription_started_at', 'Début abonnement', 'datetime'),
      field('subscription_expires_at', 'Expiration abonnement', 'datetime'),
      geoPolygonField(
        'boundary',
        'Contour de la commune',
        'Géographie',
        'Tracez le périmètre administratif de la commune.',
      ),
    ],
  },
  users: {
    key: 'users',
    title: 'Utilisateurs',
    description: 'Comptes applicatifs, rôles et rattachement communal.',
    endpoint: '/api/v1/users',
    editable: true,
    photoEndpoint: (id) => `/api/v1/users/${id}/photo`,
    columns: ['email', 'full_name', 'roles', 'commune_id', 'active'],
    secondaryColumns: ['created_at'],
    detailFields: ['email', 'full_name', 'roles', 'commune_id', 'active', 'created_at'],
    displayRelations: { commune_id: communeRelation },
    detail: {
      summaryFields: ['email', 'roles', 'commune_id'],
      related: [{ key: 'agents', title: 'Agent lié', foreignKey: 'user_id' }],
    },
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [relationFilter('commune_id', 'Commune', communeRelation), activeFilter()],
    createFields: [
      field('email', 'Email', 'email', true),
      field('password', 'Mot de passe initial', 'password', true),
      field('full_name', 'Nom complet', 'text', true),
      ...communeCascadeFields(true),
      selectMultiField(
        'roles',
        'Rôles',
        true,
        [
          option('SUPER_ADMIN', 'Super admin'),
          option('ADMIN_COMMUNE', 'Admin commune'),
          option('APM_AGENT', 'Agent APM'),
          option('SUPERVISEUR', 'Superviseur'),
          option('RECEVEUR', 'Receveur'),
        ],
        'Un utilisateur peut cumuler plusieurs rôles (Ctrl/Cmd + clic pour la sélection multiple).',
      ),
      field('active', 'Compte actif', 'checkbox'),
    ],
    patchFields: [
      field('email', 'Email', 'email', true),
      field(
        'password',
        'Nouveau mot de passe',
        'password',
        false,
        undefined,
        'Laisser vide pour conserver le mot de passe actuel.',
      ),
      field('full_name', 'Nom complet', 'text', true),
      ...communeCascadeFields(false),
      selectMultiField(
        'roles',
        'Rôles',
        true,
        [
          option('SUPER_ADMIN', 'Super admin'),
          option('ADMIN_COMMUNE', 'Admin commune'),
          option('APM_AGENT', 'Agent APM'),
          option('SUPERVISEUR', 'Superviseur'),
          option('RECEVEUR', 'Receveur'),
        ],
        'Un utilisateur peut cumuler plusieurs rôles (Ctrl/Cmd + clic pour la sélection multiple).',
      ),
      field('active', 'Compte actif', 'checkbox'),
    ],
  },
  agents: {
    key: 'agents',
    title: 'Agents',
    description: 'Agents APM, statut opérationnel et rattachement communal.',
    endpoint: '/api/v1/agents',
    editable: true,
    photoEndpoint: (id) => `/api/v1/agents/${id}/photo`,
    columns: ['matricule', 'full_name', 'commune_id', 'status'],
    detailFields: ['matricule', 'full_name', 'commune_id', 'status'],
    displayRelations: { commune_id: communeRelation },
    detail: {
      summaryFields: ['matricule', 'commune_id', 'status'],
      related: [{ key: 'pvs', title: 'PV émis', foreignKey: 'agent_id' }],
    },
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [
      statusFilter(statusOptions['agents']),
      relationFilter('commune_id', "Commune d'attache", communeRelation),
    ],
    createFields: [
      field('matricule', 'Matricule', 'text', true, 'APM-YDE1-001', undefined, 'Identité'),
      field('full_name', 'Nom complet', 'text', true, undefined, undefined, 'Identité'),
      ...communeCascadeFields(true, 'Affectation', "Commune d'attache"),
    ],
    actions: [
      sensitiveAction(
        'Suspendre',
        (row) => `/api/v1/agents/${row['id']}/suspend`,
        'Suspendre cet agent ?',
      ),
      sensitiveAction(
        'Réactiver',
        (row) => `/api/v1/agents/${row['id']}/reactivate`,
        'Réactiver cet agent ?',
      ),
      sensitiveAction(
        'Retraite',
        (row) => `/api/v1/agents/${row['id']}/retire`,
        'Mettre cet agent à la retraite ?',
      ),
    ],
  },
  zones: {
    key: 'zones',
    title: 'Zones',
    description: 'Quartiers, secteurs, marchés et zones sensibles par commune.',
    endpoint: '/api/v1/zones',
    editable: true,
    columns: ['nom', 'type_zone', 'commune_id', 'parent_id', 'active'],
    detailFields: ['nom', 'type_zone', 'commune_id', 'parent_id', 'active'],
    displayRelations: { commune_id: communeRelation, parent_id: zoneRelation },
    detail: {
      summaryFields: ['type_zone', 'commune_id', 'active'],
      related: [
        { key: 'pvs', title: 'PV dans la zone', foreignKey: 'zone_id' },
        { key: 'patrouilles', title: 'Patrouilles', foreignKey: 'zone_id' },
      ],
    },
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [relationFilter('commune_id', 'Commune', communeRelation), activeFilter()],
    createFields: [
      ...communeCascadeFields(true),
      field('nom', 'Nom de zone', 'text', true),
      selectField('type_zone', 'Type de zone', true, [
        option('QUARTIER', 'Quartier'),
        option('BLOC', 'Bloc'),
        option('SECTEUR', 'Secteur'),
        option('MARCHE', 'Marché'),
        option('ZONE_SENSIBLE', 'Zone sensible'),
      ]),
      relationField('parent_id', 'Zone parente', zoneRelation, false, undefined, 'commune_id'),
      field('active', 'Zone active', 'checkbox'),
      geoPolygonField(
        'boundary',
        'Contour de la zone',
        'Géographie',
        'Tracez le contour du quartier / secteur sur la carte.',
      ),
    ],
    patchFields: [
      field('nom', 'Nom de zone', 'text', true),
      selectField('type_zone', 'Type de zone', true, [
        option('QUARTIER', 'Quartier'),
        option('BLOC', 'Bloc'),
        option('SECTEUR', 'Secteur'),
        option('MARCHE', 'Marché'),
        option('ZONE_SENSIBLE', 'Zone sensible'),
      ]),
      relationField('parent_id', 'Zone parente', zoneRelation, false),
      field('active', 'Zone active', 'checkbox'),
      geoPolygonField(
        'boundary',
        'Contour de la zone',
        'Géographie',
        'Tracez le contour du quartier / secteur sur la carte.',
      ),
    ],
    actions: [
      deleteAction('Supprimer', (row) => `/api/v1/zones/${row['id']}`, 'Supprimer cette zone ?'),
    ],
  },
  'referentiel-categories': {
    key: 'referentiel-categories',
    title: 'Catégories',
    description: 'Premier niveau du référentiel communal.',
    endpoint: '/api/v1/referentiel/categories',
    editable: true,
    columns: ['nom', 'commune_id', 'description', 'active'],
    detailFields: ['nom', 'commune_id', 'description', 'active'],
    displayRelations: { commune_id: communeRelation },
    detail: {
      summaryFields: ['nom', 'commune_id', 'active'],
      related: [{ key: 'referentiel-types', title: 'Types', foreignKey: 'category_id' }],
    },
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [relationFilter('commune_id', 'Commune', communeRelation), activeFilter()],
    createFields: [
      ...communeCascadeFields(true),
      field('nom', 'Nom catégorie', 'text', true),
      field('description', 'Description', 'textarea'),
      field('active', 'Catégorie active', 'checkbox'),
    ],
    patchFields: [
      field('nom', 'Nom catégorie', 'text', true),
      field('description', 'Description', 'textarea'),
      field('active', 'Catégorie active', 'checkbox'),
    ],
    actions: [
      deleteAction(
        'Supprimer',
        (row) => `/api/v1/referentiel/categories/${row['id']}`,
        'Supprimer cette catégorie ?',
      ),
    ],
  },
  'referentiel-types': {
    key: 'referentiel-types',
    title: 'Types intervention',
    description: 'Deuxième niveau du référentiel communal.',
    endpoint: '/api/v1/referentiel/types',
    editable: true,
    columns: ['nom', 'category_id', 'commune_id', 'description', 'active'],
    detailFields: ['nom', 'category_id', 'commune_id', 'description', 'active'],
    displayRelations: { commune_id: communeRelation, category_id: categoryRelation },
    detail: {
      summaryFields: ['nom', 'category_id', 'commune_id'],
      related: [
        { key: 'referentiel-interventions', title: 'Interventions', foreignKey: 'type_id' },
      ],
    },
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [
      relationFilter('commune_id', 'Commune', communeRelation),
      relationFilter('category_id', 'Catégorie', categoryRelation),
      activeFilter(),
    ],
    createFields: [
      ...communeCascadeFields(true, 'Cascade référentiel'),
      relationField(
        'category_id',
        'Catégorie',
        categoryRelation,
        true,
        'Cascade référentiel',
        'commune_id',
      ),
      field('nom', 'Nom type', 'text', true, undefined, undefined, 'Définition'),
      field('description', 'Description', 'textarea', false, undefined, undefined, 'Définition'),
      field('active', 'Type actif', 'checkbox', false, undefined, undefined, 'Définition'),
    ],
    patchFields: [
      relationField('category_id', 'Catégorie', categoryRelation, true, 'Cascade référentiel'),
      field('nom', 'Nom type', 'text', true, undefined, undefined, 'Définition'),
      field('description', 'Description', 'textarea', false, undefined, undefined, 'Définition'),
      field('active', 'Type actif', 'checkbox', false, undefined, undefined, 'Définition'),
    ],
    actions: [
      deleteAction(
        'Supprimer',
        (row) => `/api/v1/referentiel/types/${row['id']}`,
        'Supprimer ce type ?',
      ),
    ],
  },
  'referentiel-interventions': {
    key: 'referentiel-interventions',
    title: 'Interventions',
    description: 'Montants, délais, pénalités et références de délibération.',
    endpoint: '/api/v1/referentiel/interventions',
    editable: true,
    columns: ['nom', 'type_id', 'sujet_paiement', 'montant_fcfa', 'delai_paiement_jours', 'active'],
    secondaryColumns: ['reference_deliberation'],
    detailFields: [
      'nom',
      'commune_id',
      'category_id',
      'type_id',
      'description',
      'requires_vehicle',
      'sujet_paiement',
      'montant_fcfa',
      'delai_paiement_jours',
      'taux_penalite_basis_points',
      'penalite_fcfa',
      'reference_deliberation',
      'active',
    ],
    displayRelations: {
      commune_id: communeRelation,
      category_id: categoryRelation,
      type_id: typeRelation,
    },
    detail: {
      summaryFields: ['nom', 'type_id', 'montant_fcfa'],
    },
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [
      relationFilter('commune_id', 'Commune', communeRelation),
      relationFilter('category_id', 'Catégorie', categoryRelation),
      relationFilter('type_id', 'Type', typeRelation),
      activeFilter(),
    ],
    createFields: [
      ...communeCascadeFields(true, 'Cascade référentiel'),
      relationField(
        'category_id',
        'Catégorie',
        categoryRelation,
        false,
        'Cascade référentiel',
        'commune_id',
      ),
      relationField(
        'type_id',
        'Type intervention',
        typeRelation,
        true,
        'Cascade référentiel',
        'category_id',
      ),
      field('nom', 'Nom intervention', 'text', true, undefined, undefined, 'Règle financière'),
      field(
        'description',
        'Description',
        'textarea',
        false,
        undefined,
        undefined,
        'Règle financière',
      ),
      field(
        'requires_vehicle',
        'Véhicule requis',
        'checkbox',
        false,
        undefined,
        'Affiche les champs véhicule par défaut sur mobile.',
        'Règle financière',
      ),
      field(
        'requires_vehicle',
        'Véhicule requis',
        'checkbox',
        false,
        undefined,
        'Affiche les champs véhicule par défaut sur mobile.',
        'Règle financière',
      ),
      field(
        'sujet_paiement',
        'Sujet à paiement',
        'checkbox',
        false,
        undefined,
        'Désactivé pour un avertissement ou une intervention non payante.',
        'Règle financière',
      ),
      field(
        'montant_fcfa',
        'Montant FCFA',
        'money',
        false,
        undefined,
        'Montant officiel issu de la délibération.',
        'Règle financière',
      ),
      field(
        'delai_paiement_jours',
        'Délai paiement',
        'number',
        false,
        '30',
        'Nombre de jours avant pénalité.',
        'Règle financière',
      ),
      field(
        'taux_penalite_basis_points',
        'Pénalité (taux)',
        'number',
        false,
        '500',
        'Basis points: 500 = 5%. Ignoré si une pénalité forfaitaire est définie.',
        'Règle financière',
      ),
      field(
        'penalite_fcfa',
        'Pénalité forfaitaire (FCFA)',
        'money',
        false,
        undefined,
        'Montant fixe délibéré par la commune. Si > 0, remplace le taux ; 0 ou vide = pénalité au taux.',
        'Règle financière',
      ),
      field(
        'reference_deliberation',
        'Référence délibération',
        'text',
        false,
        undefined,
        undefined,
        'Règle financière',
      ),
      field(
        'active',
        'Intervention active',
        'checkbox',
        false,
        undefined,
        undefined,
        'Règle financière',
      ),
    ],
    patchFields: [
      relationField('type_id', 'Type intervention', typeRelation, true, 'Cascade référentiel'),
      field('nom', 'Nom intervention', 'text', true, undefined, undefined, 'Règle financière'),
      field(
        'description',
        'Description',
        'textarea',
        false,
        undefined,
        undefined,
        'Règle financière',
      ),
      field(
        'sujet_paiement',
        'Sujet à paiement',
        'checkbox',
        false,
        undefined,
        'Désactivé pour un avertissement ou une intervention non payante.',
        'Règle financière',
      ),
      field(
        'montant_fcfa',
        'Montant FCFA',
        'money',
        false,
        undefined,
        'Montant officiel issu de la délibération.',
        'Règle financière',
      ),
      field(
        'delai_paiement_jours',
        'Délai paiement',
        'number',
        false,
        '30',
        'Nombre de jours avant pénalité.',
        'Règle financière',
      ),
      field(
        'taux_penalite_basis_points',
        'Pénalité (taux)',
        'number',
        false,
        '500',
        'Basis points: 500 = 5%. Ignoré si une pénalité forfaitaire est définie.',
        'Règle financière',
      ),
      field(
        'penalite_fcfa',
        'Pénalité forfaitaire (FCFA)',
        'money',
        false,
        undefined,
        'Montant fixe délibéré par la commune. Si > 0, remplace le taux ; 0 ou vide = pénalité au taux.',
        'Règle financière',
      ),
      field(
        'reference_deliberation',
        'Référence délibération',
        'text',
        false,
        undefined,
        undefined,
        'Règle financière',
      ),
      field(
        'active',
        'Intervention active',
        'checkbox',
        false,
        undefined,
        undefined,
        'Règle financière',
      ),
    ],
    actions: [
      deleteAction(
        'Supprimer',
        (row) => `/api/v1/referentiel/interventions/${row['id']}`,
        'Supprimer cette intervention ?',
      ),
    ],
  },
  pvs: {
    key: 'pvs',
    title: 'Procès-verbaux',
    description: 'Création, suivi, QR code et impression des PV.',
    endpoint: '/api/v1/pvs',
    columns: [
      'pv_number',
      'status',
      'subject_type',
      'interventions',
      'amount_initial_fcfa',
      'created_at',
    ],
    secondaryColumns: [
      'vehicle_plate',
      'vehicle_registration_card_number',
      'verbalized_name',
      'verbalized_identity_number',
      'zone_id',
    ],
    detailFields: [
      'pv_number',
      'status',
      'subject_type',
      'subject_kind',
      'raison_sociale',
      'interventions',
      'amount_initial_fcfa',
      'vehicle_plate',
      'vehicle_registration_card_number',
      'vehicle_make',
      'vehicle_model',
      'vehicle_color',
      'vehicle_owner_name',
      'verbalized_name',
      'verbalized_identifier',
      'verbalized_first_name',
      'verbalized_last_name',
      'verbalized_identity_type',
      'verbalized_identity_number',
      'verbalized_phone',
      'verbalized_address',
      'zone_id',
      'location_description',
      'gps_latitude',
      'gps_longitude',
      'notes_internes',
      'created_at',
    ],
    displayRelations: { zone_id: zoneRelation, agent_id: agentRelation },
    detail: {
      summaryFields: ['pv_number', 'status', 'amount_initial_fcfa'],
    },
    labels: commonLabels(),
    createRoles: ['APM_AGENT'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE', 'SUPERVISEUR'],
    filters: [
      statusFilter(statusOptions['pvs']),
      relationFilter('agent_id', 'Agent', {
        endpoint: '/api/v1/agents',
        labelKey: 'full_name',
        metaKey: 'matricule',
        statusKey: 'status',
      }),
    ],
    editable: true,
    createFields: [
      selectField(
        'subject_type',
        'Type de PV',
        true,
        pvSubjectTypeOptions,
        'Le backend exige ensuite les champs usager/véhicule cohérents avec ce type.',
        '1. Type',
      ),
      relationMultiField(
        'intervention_ids',
        'Infractions',
        interventionRelation,
        true,
        '2. Infractions',
      ),
      {
        ...selectField(
          'subject_kind',
          'Type de personne',
          true,
          [option('PHYSIQUE', 'Personne physique'), option('MORALE', 'Personne morale')],
          'Une personne morale est identifiée par sa raison sociale.',
          '3. Contrevenant',
        ),
        default: 'PHYSIQUE',
      },
      {
        ...field(
          'raison_sociale',
          'Raison sociale',
          'text',
          true,
          'Ets CAMERAMAN',
          undefined,
          '3. Contrevenant',
        ),
        visibleWhen: { field: 'subject_kind', equals: 'MORALE' },
      },
      {
        ...field('verbalized_last_name', 'Nom', 'text', true, undefined, undefined, '3. Contrevenant'),
        visibleWhen: { field: 'subject_kind', equals: 'PHYSIQUE' },
      },
      {
        ...field(
          'verbalized_first_name',
          'Prénom',
          'text',
          false,
          undefined,
          undefined,
          '3. Contrevenant',
        ),
        visibleWhen: { field: 'subject_kind', equals: 'PHYSIQUE' },
      },
      selectField(
        'verbalized_identity_type',
        "Type d'identité",
        false,
        [
          option('CNI', 'CNI'),
          option('PASSEPORT', 'Passeport'),
          option('PERMIS_CONDUIRE', 'Permis de conduire'),
          option('CARTE_SEJOUR', 'Carte de séjour'),
          option('NIU', 'NIU'),
          option('AUTRE', 'Autre'),
        ],
        "Requis si un numéro d'identité est renseigné.",
        '3. Contrevenant',
      ),
      field(
        'verbalized_identity_number',
        "Numéro d'identité",
        'text',
        false,
        'Numéro CNI, passeport, permis...',
        undefined,
        '3. Contrevenant',
      ),
      field(
        'verbalized_phone',
        'Téléphone',
        'text',
        true,
        undefined,
        undefined,
        '3. Contrevenant',
      ),
      field(
        'verbalized_address',
        'Adresse',
        'text',
        false,
        undefined,
        undefined,
        '3. Contrevenant',
      ),
      field('vehicle_plate', 'Plaque véhicule', 'text', false, undefined, undefined, '4. Véhicule'),
      field(
        'vehicle_registration_card_number',
        'Numéro carte grise',
        'text',
        false,
        undefined,
        'Alternative à la plaque pour identifier le véhicule.',
        '4. Véhicule',
      ),
      field('vehicle_make', 'Marque', 'text', false, undefined, undefined, '4. Véhicule'),
      field('vehicle_model', 'Modèle', 'text', false, undefined, undefined, '4. Véhicule'),
      field('vehicle_color', 'Couleur', 'text', false, undefined, undefined, '4. Véhicule'),
      field(
        'vehicle_owner_name',
        'Propriétaire',
        'text',
        false,
        undefined,
        undefined,
        '4. Véhicule',
      ),
      relationField('zone_id', 'Zone', zoneRelation, false, '5. Localisation'),
      field(
        'location_description',
        'Lieu',
        'textarea',
        false,
        undefined,
        undefined,
        '5. Localisation',
      ),
      geoPointField(
        'gps_latitude',
        'gps_longitude',
        'Position GPS',
        '5. Localisation',
        'Cliquez sur la carte ou recherchez une adresse. La zone est déduite automatiquement.',
      ),
      field(
        'notes_internes',
        'Notes internes',
        'textarea',
        false,
        undefined,
        undefined,
        '6. Récapitulatif',
      ),
    ],
    patchFields: [
      selectField(
        'subject_type',
        'Type de PV',
        true,
        pvSubjectTypeOptions,
        "Modification refusée par l'API si le PV est payé ou annulé.",
        '1. Type',
      ),
      relationMultiField(
        'intervention_ids',
        'Infractions',
        interventionRelation,
        true,
        '2. Infractions',
      ),
      {
        ...selectField(
          'subject_kind',
          'Type de personne',
          true,
          [option('PHYSIQUE', 'Personne physique'), option('MORALE', 'Personne morale')],
          'Une personne morale est identifiée par sa raison sociale.',
          '3. Contrevenant',
        ),
        default: 'PHYSIQUE',
      },
      {
        ...field(
          'raison_sociale',
          'Raison sociale',
          'text',
          true,
          'Ets CAMERAMAN',
          undefined,
          '3. Contrevenant',
        ),
        visibleWhen: { field: 'subject_kind', equals: 'MORALE' },
      },
      {
        ...field(
          'verbalized_last_name',
          'Nom',
          'text',
          false,
          undefined,
          undefined,
          '3. Contrevenant',
        ),
        visibleWhen: { field: 'subject_kind', equals: 'PHYSIQUE' },
      },
      {
        ...field(
          'verbalized_first_name',
          'Prénom',
          'text',
          false,
          undefined,
          undefined,
          '3. Contrevenant',
        ),
        visibleWhen: { field: 'subject_kind', equals: 'PHYSIQUE' },
      },
      selectField(
        'verbalized_identity_type',
        "Type d'identité",
        false,
        [
          option('CNI', 'CNI'),
          option('PASSEPORT', 'Passeport'),
          option('PERMIS_CONDUIRE', 'Permis de conduire'),
          option('CARTE_SEJOUR', 'Carte de séjour'),
          option('NIU', 'NIU'),
          option('AUTRE', 'Autre'),
        ],
        "Requis si un numéro d'identité est renseigné.",
        '3. Contrevenant',
      ),
      field(
        'verbalized_identity_number',
        "Numéro d'identité",
        'text',
        false,
        'Numéro CNI, passeport, permis...',
        undefined,
        '3. Contrevenant',
      ),
      field(
        'verbalized_phone',
        'Téléphone',
        'text',
        false,
        undefined,
        undefined,
        '3. Contrevenant',
      ),
      field(
        'verbalized_address',
        'Adresse',
        'text',
        false,
        undefined,
        undefined,
        '3. Contrevenant',
      ),
      field('vehicle_plate', 'Plaque véhicule', 'text', false, undefined, undefined, '4. Véhicule'),
      field(
        'vehicle_registration_card_number',
        'Numéro carte grise',
        'text',
        false,
        undefined,
        'Alternative à la plaque pour identifier le véhicule.',
        '4. Véhicule',
      ),
      field('vehicle_make', 'Marque', 'text', false, undefined, undefined, '4. Véhicule'),
      field('vehicle_model', 'Modèle', 'text', false, undefined, undefined, '4. Véhicule'),
      field('vehicle_color', 'Couleur', 'text', false, undefined, undefined, '4. Véhicule'),
      field(
        'vehicle_owner_name',
        'Propriétaire',
        'text',
        false,
        undefined,
        undefined,
        '4. Véhicule',
      ),
      relationField('zone_id', 'Zone', zoneRelation, false, '5. Localisation'),
      field(
        'location_description',
        'Lieu',
        'textarea',
        false,
        undefined,
        undefined,
        '5. Localisation',
      ),
      geoPointField(
        'gps_latitude',
        'gps_longitude',
        'Position GPS',
        '5. Localisation',
        'Cliquez sur la carte ou recherchez une adresse. La zone est déduite automatiquement.',
      ),
      field(
        'notes_internes',
        'Notes internes',
        'textarea',
        false,
        undefined,
        undefined,
        '6. Récapitulatif',
      ),
    ],
    actions: [
      {
        label: 'QR',
        kind: 'download',
        path: (row) => `/api/v1/pvs/${row['id']}/qr`,
        filename: (row) => `qr-${row['pv_number']}.svg`,
      },
      {
        label: 'PDF',
        kind: 'download',
        path: (row) => `/api/v1/pvs/${row['id']}/pdf`,
        filename: (row) => `${row['pv_number']}.pdf`,
      },
      {
        // Partage natif (Web Share API) : envoie le PV vers un numéro via le sélecteur du
        // device, en attendant l'API WhatsApp. Le lien de suivi est celui encodé dans le QR.
        label: 'Partager',
        kind: 'share',
        path: (row) => `/api/v1/pvs/${row['id']}/pdf`,
        filename: (row) => `${row['pv_number']}.pdf`,
        shareUrl: (row) => `${apiBaseUrl()}/api/v1/public/pvs/${row['pv_number']}`,
        shareText: (row) => `Procès-verbal ${row['pv_number']} — suivi et paiement`,
      },
      statusAction({
        label: 'Changer statut',
        path: (row) => `/api/v1/pvs/${row['id']}/status`,
        options: statusOptions['pvs'],
        transitions: {
          BROUILLON: ['EMIS', 'ANNULE'],
          EMIS: ['EN_ATTENTE_PAIEMENT', 'ANNULE'],
          EN_ATTENTE_PAIEMENT: ['PAYE', 'EN_RETARD', 'ANNULE', 'CONTESTE'],
          EN_RETARD: ['PAYE', 'ANNULE', 'CONTESTE'],
          CONTESTE: ['EN_ATTENTE_PAIEMENT', 'ANNULE'],
          PAYE: [],
          ANNULE: [],
          NON_PAYANT: [],
        },
        extra: [
          {
            key: 'reason',
            label: 'Motif',
            type: 'textarea',
            placeholder: 'Motif du changement de statut (journalisé).',
          },
        ],
        roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
      }),
      deleteAction('Annuler le PV', (row) => `/api/v1/pvs/${row['id']}`, 'Annuler ce PV ?', [
        'SUPER_ADMIN',
        'ADMIN_COMMUNE',
      ]),
    ],
  },
  signalements: {
    key: 'signalements',
    title: 'Signalements',
    description: 'Signalements citoyens, priorisation et suivi administratif.',
    endpoint: '/api/v1/signalements',
    columns: [
      'signalement_number',
      'type_incident',
      'location_description',
      'status',
      'created_at',
    ],
    secondaryColumns: ['description'],
    detailFields: [
      'signalement_number',
      'type_incident',
      'location_description',
      'description',
      'status',
      'escalation_target',
      'escalated_at',
      'contact_anonyme',
      'created_at',
      'updated_at',
    ],
    detail: {
      summaryFields: ['signalement_number', 'type_incident', 'status'],
    },
    labels: commonLabels(),
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [
      statusFilter(statusOptions['signalements']),
      relationFilter('commune_id', 'Commune', communeRelation),
    ],
    actions: [
      statusAction({
        label: 'Traiter',
        path: (row) => `/api/v1/signalements/${row['id']}/status`,
        options: statusOptions['signalements'],
        extra: [
          {
            key: 'admin_notes',
            label: 'Note administrative',
            type: 'textarea',
            placeholder: 'Suivi interne (visible back-office uniquement).',
          },
          {
            key: 'assigned_to',
            label: 'Affecter à',
            type: 'relation',
            relation: userRelation,
            // Restreint l'affectation aux profils actifs de la commune du
            // signalement + superviseurs globaux (NASLA / MINISTÈRE).
            rowQuery: (row) => ({
              commune_id: String(row['commune_id'] ?? ''),
              include_global: true,
              active: true,
            }),
          },
        ],
        roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
      }),
      escalateAction({
        path: (row) => `/api/v1/signalements/${row['id']}/escalate`,
        roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
      }),
    ],
  },
  fourrieres: {
    key: 'fourrieres',
    title: 'Fourrières',
    description: 'Mises en fourrière (véhicules et autres objets), gardiennage et restitution.',
    endpoint: '/api/v1/fourrieres',
    columns: ['fourriere_number', 'pv_number', 'item_type', 'designation', 'vehicle_plate', 'status', 'entered_at'],
    secondaryColumns: ['motif'],
    detailFields: [
      'fourriere_number',
      'pv_number',
      'item_type',
      'designation',
      'vehicle_plate',
      'vehicle_type',
      'vehicle_details',
      'motif',
      'lieu_enlevement',
      'status',
      'daily_fee_fcfa',
      'frais_gardiennage_fcfa',
      'entered_at',
      'released_at',
      'released_to',
      'commune_id',
      'created_at',
      'updated_at',
    ],
    displayRelations: { commune_id: communeRelation },
    detail: {
      summaryFields: ['fourriere_number', 'item_type', 'status'],
    },
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [
      statusFilter(statusOptions['fourrieres']),
      relationFilter('commune_id', 'Commune', communeRelation),
    ],
    createFields: [
      ...communeCascadeFields(true),
      {
        ...relationField('pv_id', 'PV existant (optionnel)', pvRelation, false, undefined, 'commune_id'),
        help: "PV déjà dressé pour cette infraction. Laisser vide pour générer automatiquement le PV de mise en fourrière.",
      },
      {
        ...relationField('agent_id', 'Agent ayant procédé', activeAgentRelation, false, undefined, 'commune_id'),
        help: "Requis si aucun PV existant n'est lié : le PV de mise en fourrière est généré au nom de cet agent.",
      },
      {
        ...selectField(
          'item_type',
          "Type d'objet",
          true,
          [
            option('VEHICULE', 'Véhicule'),
            option('ENGIN', 'Engin / matériel'),
            option('MARCHANDISE', 'Marchandise saisie'),
            option('ANIMAL', 'Animal'),
            option('AUTRE', 'Autre'),
          ],
          undefined,
        ),
        default: 'VEHICULE',
      },
      {
        ...field(
          'designation',
          'Désignation',
          'text',
          true,
          "Libellé de l'objet mis en fourrière.",
        ),
        visibleWhen: { field: 'item_type', equals: ['ENGIN', 'MARCHANDISE', 'ANIMAL', 'AUTRE'] },
      },
      {
        ...field('vehicle_plate', 'Plaque', 'text', true),
        visibleWhen: { field: 'item_type', equals: 'VEHICULE' },
      },
      {
        ...field('vehicle_type', 'Type de véhicule', 'text', false, 'Berline, moto, camion…'),
        visibleWhen: { field: 'item_type', equals: 'VEHICULE' },
      },
      {
        ...field('vehicle_details', 'Détails véhicule', 'textarea'),
        visibleWhen: { field: 'item_type', equals: 'VEHICULE' },
      },
      field('motif', 'Motif', 'textarea', true, "Raison de la mise en fourrière."),
      field('lieu_enlevement', "Lieu d'enlèvement", 'text'),
      field('daily_fee_fcfa', 'Tarif journalier FCFA', 'money', false, 'Frais de gardiennage / jour.'),
    ],
    actions: [
      statusAction({
        label: 'Changer le statut',
        path: (row) => `/api/v1/fourrieres/${row['id']}/status`,
        options: statusOptions['fourrieres'],
        extra: [
          {
            key: 'released_to',
            label: 'Restitué à',
            type: 'text',
            placeholder: 'Nom du propriétaire (en cas de restitution).',
          },
        ],
        roles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
      }),
    ],
  },
  patrouilles: {
    key: 'patrouilles',
    title: 'Patrouilles',
    description: 'Planification, démarrage et clôture des patrouilles.',
    endpoint: '/api/v1/patrouilles',
    editable: true,
    columns: ['nom', 'status', 'zone_id', 'date_debut', 'date_fin'],
    secondaryColumns: ['description'],
    detailFields: [
      'nom',
      'description',
      'commune_id',
      'zone_id',
      'status',
      'date_debut_prevue',
      'date_fin_prevue',
      'date_debut',
      'date_fin',
    ],
    displayRelations: { commune_id: communeRelation, zone_id: zoneRelation },
    detail: {
      summaryFields: ['nom', 'status', 'zone_id'],
    },
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [
      statusFilter(statusOptions['patrouilles']),
      relationFilter('zone_id', 'Zone', zoneRelation),
    ],
    manageAgents: true,
    createFields: [
      ...communeCascadeFields(true),
      relationField('zone_id', 'Zone', zoneRelation, true, undefined, 'commune_id'),
      field('nom', 'Nom patrouille', 'text', false, undefined, 'Auto-généré si vide.'),
      field('description', 'Description', 'textarea'),
      field('date_debut_prevue', 'Début prévu', 'datetime'),
      field('date_fin_prevue', 'Fin prévue', 'datetime'),
      relationMultiField('agent_ids', 'Agents affectés', activeAgentRelation, true, undefined, 'commune_id'),
    ],
    patchFields: [
      relationField('zone_id', 'Zone', zoneRelation, false),
      field('nom', 'Nom patrouille', 'text', true),
      field('description', 'Description', 'textarea'),
      field('date_debut_prevue', 'Début prévu', 'datetime'),
      field('date_fin_prevue', 'Fin prévue', 'datetime'),
    ],
    actions: [
      sensitiveAction(
        'Démarrer',
        (row) => `/api/v1/patrouilles/${row['id']}/start`,
        'Démarrer cette patrouille ?',
      ),
      sensitiveAction(
        'Clôturer',
        (row) => `/api/v1/patrouilles/${row['id']}/end`,
        'Clôturer cette patrouille ?',
      ),
    ],
  },
  'audit-logs': {
    key: 'audit-logs',
    title: 'Audit logs',
    description: 'Journal des actions sensibles.',
    endpoint: '/api/v1/audit-logs',
    columns: ['action', 'entity_type', 'entity_id', 'user_id', 'created_at'],
    detailFields: [
      'action',
      'entity_type',
      'entity_id',
      'user_id',
      'commune_id',
      'ip_address',
      'user_agent',
      'created_at',
    ],
    displayRelations: { user_id: userRelation, commune_id: communeRelation },
    detail: {
      summaryFields: ['action', 'entity_type', 'user_id'],
    },
    labels: commonLabels(),
    filters: [relationFilter('commune_id', 'Commune', communeRelation)],
  },
};

function field(
  key: string,
  label: string,
  type: FieldType,
  required = false,
  placeholder?: string,
  help?: string,
  section?: string,
): ResourceField {
  return { key, label, type, required, placeholder, help, section };
}

function relationField(
  key: string,
  label: string,
  relation: RelationConfig,
  required = true,
  section?: string,
  dependsOn?: string,
): ResourceField {
  return { key, label, type: 'relation', required, relation, section, dependsOn };
}

function relationMultiField(
  key: string,
  label: string,
  relation: RelationConfig,
  required = true,
  section?: string,
  dependsOn?: string,
): ResourceField {
  return { key, label, type: 'relation_multi', required, relation, section, dependsOn };
}

/**
 * Sélection géographique hiérarchique Région → Département → Commune.
 *
 * Les champs `region_id` et `departement_id` sont purement UI (`uiOnly`, jamais envoyés) :
 * ils ne servent qu'à filtrer la liste des communes. Seul `commune_id` est persisté.
 * À utiliser dans tout formulaire où l'on choisit une commune à un niveau libre.
 */
function communeCascadeFields(
  communeRequired = false,
  section?: string,
  communeLabel = 'Commune',
): ResourceField[] {
  return [
    { ...relationField('region_id', 'Région', regionRelation, false, section), uiOnly: true },
    {
      ...relationField(
        'departement_id',
        'Département',
        departementRelation,
        false,
        section,
        'region_id',
      ),
      uiOnly: true,
    },
    relationField('commune_id', communeLabel, communeRelation, communeRequired, section, 'departement_id'),
  ];
}

function selectField(
  key: string,
  label: string,
  required: boolean,
  options: SelectOption[],
  help?: string,
  section?: string,
): ResourceField {
  return { key, label, type: 'select', required, options, help, section };
}

function selectMultiField(
  key: string,
  label: string,
  required: boolean,
  options: SelectOption[],
  help?: string,
  section?: string,
): ResourceField {
  return { key, label, type: 'select_multi', required, options, help, section };
}

/** Sélecteur de position : un champ carte pilotant deux clés lat/lon du formulaire. */
function geoPointField(
  latKey: string,
  lonKey: string,
  label: string,
  section?: string,
  help?: string,
): ResourceField {
  return { key: latKey, label, type: 'geopoint', latKey, lonKey, section, help };
}

/** Éditeur de contour : un champ carte pilotant une clé GeoJSON (boundary). */
function geoPolygonField(
  key: string,
  label: string,
  section?: string,
  help?: string,
): ResourceField {
  return { key, label, type: 'geopolygon', section, help };
}

function option(value: string | number | boolean, label: string): SelectOption {
  return { value, label };
}

function statusFilter(options: SelectOption[]): ResourceFilter {
  return { key: 'status', label: 'Statut', type: 'status', options };
}

function activeFilter(): ResourceFilter {
  return {
    key: 'active',
    label: 'État',
    type: 'active',
    options: [option(true, 'Actif'), option(false, 'Inactif')],
  };
}

function relationFilter(key: string, label: string, relation: RelationConfig): ResourceFilter {
  return { key, label, type: 'relation', relation };
}

function sensitiveAction(
  label: string,
  path: (row: Record<string, unknown>) => string,
  confirmTitle: string,
): ResourceAction {
  return {
    label,
    kind: 'post',
    path,
    sensitive: true,
    confirmTitle,
    confirmMessage: (row) => {
      const target = String(
        row['full_name'] ?? row['nom'] ?? row['pv_number'] ?? row['id'] ?? 'cet élément',
      );
      return `Cette action modifie un statut sensible pour ${target}. Elle sera journalisée.`;
    },
  };
}

function deleteAction(
  label: string,
  path: (row: Record<string, unknown>) => string,
  confirmTitle: string,
  roles?: RoleCode[],
): ResourceAction {
  return {
    label,
    kind: 'delete',
    path,
    sensitive: true,
    confirmTitle,
    roles,
    confirmMessage: (row) => {
      const target = String(
        row['full_name'] ?? row['nom'] ?? row['pv_number'] ?? row['id'] ?? 'cet élément',
      );
      return `Suppression définitive de ${target}. Cette action est journalisée et irréversible côté liste.`;
    },
  };
}

function statusAction(config: {
  label: string;
  path: (row: Record<string, unknown>) => string;
  options: SelectOption[];
  transitions?: Record<string, string[]>;
  extra?: StatusExtraField[];
  roles?: RoleCode[];
}): ResourceAction {
  return {
    label: config.label,
    kind: 'status',
    path: config.path,
    sensitive: true,
    roles: config.roles,
    statusOptions: config.options,
    statusTransitions: config.transitions,
    statusExtra: config.extra,
  };
}

const escalationTargetOptions: SelectOption[] = [
  option('MAIRIE', 'Mairie'),
  option('NASLA', 'NASLA'),
  option('MINDDEVEL', 'MINDDEVEL'),
  option('MINAT', 'MINAT'),
];

/** Escalade d'un signalement vers une autorité de tutelle (POST /escalate). */
function escalateAction(config: {
  path: (row: Record<string, unknown>) => string;
  roles?: RoleCode[];
}): ResourceAction {
  return {
    label: 'Escalader',
    kind: 'status',
    path: config.path,
    sensitive: true,
    roles: config.roles,
    method: 'post',
    statusKey: 'target',
    statusFromKey: 'escalation_target',
    selectLabel: 'Autorité destinataire',
    currentLabel: 'Escalade actuelle',
    successMessage: 'Signalement escaladé.',
    statusOptions: escalationTargetOptions,
    statusExtra: [
      {
        key: 'note',
        label: 'Note de transmission',
        type: 'textarea',
        placeholder: 'Précisions transmises à l’autorité (journalisé).',
      },
    ],
  };
}

function commonLabels(): Record<string, string> {
  return {
    id: 'ID',
    code: 'Code',
    nom: 'Nom',
    full_name: 'Nom complet',
    email: 'Email',
    roles: 'Rôles',
    commune_id: 'Commune',
    active: 'Actif',
    subscription_status: 'Abonnement',
    subscription_started_at: 'Début abonnement',
    subscription_expires_at: 'Expiration abonnement',
    subscription_active: 'Abonnement valide',
    public_visible: 'Visible public',
    status: 'Statut',
    escalation_target: 'Escaladé vers',
    escalated_at: "Date d'escalade",
    fourriere_number: 'Numéro fourrière',
    vehicle_type: 'Type de véhicule',
    vehicle_details: 'Détails véhicule',
    motif: 'Motif',
    lieu_enlevement: "Lieu d'enlèvement",
    daily_fee_fcfa: 'Tarif journalier FCFA',
    frais_gardiennage_fcfa: 'Frais de gardiennage FCFA',
    entered_at: 'Entrée en fourrière',
    released_at: 'Date de sortie',
    released_to: 'Restitué à',
    matricule: 'Matricule',
    date_prise_fonction: 'Prise de fonction',
    region: 'Région',
    departement: 'Département',
    adresse: 'Adresse',
    telephone: 'Téléphone',
    theme_color: 'Couleur thème',
    type_zone: 'Type',
    parent_id: 'Zone parente',
    description: 'Description',
    category_id: 'Catégorie',
    type_id: 'Type',
    intervention_id: 'Intervention',
    intervention_ids: 'Infractions',
    interventions: 'Infractions',
    subject_type: 'Type PV',
    subject_kind: 'Type de personne',
    raison_sociale: 'Raison sociale',
    item_type: "Type d'objet",
    designation: 'Désignation',
    sujet_paiement: 'Payant',
    montant_fcfa: 'Montant FCFA',
    amount_initial_fcfa: 'Montant FCFA',
    amount_paid_fcfa: 'Montant encaissé',
    delai_paiement_jours: 'Délai',
    taux_penalite_basis_points: 'Pénalité',
    reference_deliberation: 'Délibération',
    pv_number: 'Numéro PV',
    verbalized_name: 'Verbalisé',
    verbalized_identifier: 'Identifiant',
    verbalized_first_name: 'Prénom',
    verbalized_last_name: 'Nom',
    verbalized_identity_type: "Type d'identité",
    verbalized_identity_number: 'Numéro identité',
    verbalized_phone: 'Téléphone',
    verbalized_address: 'Adresse',
    vehicle_plate: 'Plaque',
    vehicle_registration_card_number: 'Carte grise',
    vehicle_make: 'Marque',
    vehicle_model: 'Modèle',
    vehicle_color: 'Couleur',
    vehicle_owner_name: 'Propriétaire',
    gps_latitude: 'Latitude',
    gps_longitude: 'Longitude',
    notes_internes: 'Notes internes',
    created_at: 'Créé le',
    updated_at: 'Mis à jour',
    date_debut: 'Début',
    date_fin: 'Fin',
    date_debut_prevue: 'Début prévu',
    date_fin_prevue: 'Fin prévue',
    zone_id: 'Zone',
    signalement_number: 'Numéro',
    type_incident: 'Incident',
    location_description: 'Lieu',
    contact_anonyme: 'Anonyme',
    entity_type: 'Entité',
    entity_id: 'Entité ID',
    user_id: 'Utilisateur',
    action: 'Action',
    ip_address: 'IP',
    user_agent: 'Navigateur',
  };
}
