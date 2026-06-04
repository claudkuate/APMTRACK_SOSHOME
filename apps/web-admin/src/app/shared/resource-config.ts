import { RoleCode } from './api-types';

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
  | 'select'
  | 'relation'
  | 'status';

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
}

export interface ResourceFilter {
  key: string;
  label: string;
  type: FilterType;
  queryKey?: string;
  options?: SelectOption[];
  relation?: RelationConfig;
}

export interface ResourceAction {
  label: string;
  kind: 'download' | 'post' | 'delete';
  path: (row: Record<string, unknown>) => string;
  filename?: (row: Record<string, unknown>) => string;
  sensitive?: boolean;
  confirmTitle?: string;
  confirmMessage?: (row: Record<string, unknown>) => string;
}

export interface ResourceConfig {
  key: string;
  title: string;
  description: string;
  endpoint: string;
  columns: string[];
  secondaryColumns?: string[];
  detailFields?: string[];
  labels: Record<string, string>;
  createFields?: ResourceField[];
  patchFields?: ResourceField[];
  createRoles?: RoleCode[];
  mutateRoles?: RoleCode[];
  query?: Record<string, string | number | boolean>;
  filters?: ResourceFilter[];
  actions?: ResourceAction[];
}

const statusOptions: Record<string, SelectOption[]> = {
  agents: [
    option('ACTIF', 'Actif'),
    option('SUSPENDU', 'Suspendu'),
    option('RETRAITE', 'Retraite'),
  ],
  pvs: [
    option('EN_ATTENTE_PAIEMENT', 'En attente paiement'),
    option('PAYE', 'Paye'),
    option('EN_RETARD', 'En retard'),
    option('NON_PAYANT', 'Non payant'),
    option('ANNULE', 'Annule'),
    option('CONTESTE', 'Conteste'),
  ],
  signalements: [
    option('RECU', 'Recu'),
    option('EN_COURS', 'En cours'),
    option('TRAITE', 'Traite'),
    option('REJETE', 'Rejete'),
  ],
  patrouilles: [
    option('PLANIFIEE', 'Planifiee'),
    option('EN_COURS', 'En cours'),
    option('CLOTUREE', 'Cloturee'),
    option('ANNULEE', 'Annulee'),
  ],
};

const communeRelation: RelationConfig = {
  endpoint: '/api/v1/communes',
  labelKey: 'nom',
  metaKey: 'code',
  statusKey: 'active',
};

const zoneRelation: RelationConfig = {
  endpoint: '/api/v1/zones',
  labelKey: 'nom',
  metaKey: 'type_zone',
  statusKey: 'active',
};

const categoryRelation: RelationConfig = {
  endpoint: '/api/v1/referentiel/categories',
  labelKey: 'nom',
  metaKey: 'description',
  statusKey: 'active',
};

const typeRelation: RelationConfig = {
  endpoint: '/api/v1/referentiel/types',
  labelKey: 'nom',
  metaKey: 'description',
  statusKey: 'active',
};

const interventionRelation: RelationConfig = {
  endpoint: '/api/v1/referentiel/interventions',
  labelKey: 'nom',
  metaKey: 'montant_fcfa',
  statusKey: 'active',
};

const userRelation: RelationConfig = {
  endpoint: '/api/v1/users',
  labelKey: 'full_name',
  metaKey: 'email',
  statusKey: 'active',
};

export const resourceConfigs: Record<string, ResourceConfig> = {
  communes: {
    key: 'communes',
    title: 'Communes',
    description: 'Parametrage institutionnel et perimetres de travail.',
    endpoint: '/api/v1/communes',
    columns: ['code', 'nom', 'region', 'departement', 'active'],
    secondaryColumns: ['telephone', 'email'],
    detailFields: ['code', 'nom', 'region', 'departement', 'adresse', 'telephone', 'email', 'theme_color', 'active'],
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [activeFilter()],
    createFields: [
      field('code', 'Code commune', 'text', true, 'YDE1'),
      field('nom', 'Nom officiel', 'text', true),
      field('region', 'Region', 'text', true),
      field('departement', 'Departement', 'text', true),
      field('adresse', 'Adresse', 'text'),
      field('telephone', 'Telephone', 'text'),
      field('email', 'Email', 'email'),
      field('theme_color', 'Couleur theme', 'text', false, '#1F7A4D'),
      field('active', 'Commune active', 'checkbox'),
    ],
  },
  users: {
    key: 'users',
    title: 'Utilisateurs',
    description: 'Comptes applicatifs, roles et rattachement communal.',
    endpoint: '/api/v1/users',
    columns: ['email', 'full_name', 'roles', 'commune_id', 'active'],
    secondaryColumns: ['created_at'],
    detailFields: ['email', 'full_name', 'roles', 'commune_id', 'active', 'created_at'],
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [relationFilter('commune_id', 'Commune', communeRelation), activeFilter()],
    createFields: [
      field('email', 'Email', 'email', true),
      field('password', 'Mot de passe initial', 'password', true),
      field('full_name', 'Nom complet', 'text', true),
      relationField('commune_id', 'Commune', communeRelation),
      selectField('roles', 'Roles', true, [
        option('SUPER_ADMIN', 'Super admin'),
        option('ADMIN_COMMUNE', 'Admin commune'),
        option('APM_AGENT', 'Agent APM'),
        option('SUPERVISEUR', 'Superviseur'),
        option('RECEVEUR', 'Receveur'),
      ], 'Un seul role principal dans ce formulaire; les roles multiples restent possibles via API.'),
      field('active', 'Compte actif', 'checkbox'),
    ],
  },
  agents: {
    key: 'agents',
    title: 'Agents',
    description: 'Agents APM, statut operationnel et rattachement communal.',
    endpoint: '/api/v1/agents',
    columns: ['matricule', 'full_name', 'grade', 'status', 'formation_nasla'],
    secondaryColumns: ['commune_id', 'telephone', 'email'],
    detailFields: [
      'matricule',
      'full_name',
      'commune_id',
      'grade',
      'status',
      'date_prise_fonction',
      'formation_nasla',
      'telephone',
      'email',
      'user_id',
    ],
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [statusFilter(statusOptions['agents']), relationFilter('commune_id', 'Commune', communeRelation)],
    createFields: [
      field('matricule', 'Matricule', 'text', true, 'APM-YDE1-001', undefined, 'Identite'),
      field('full_name', 'Nom complet', 'text', true, undefined, undefined, 'Identite'),
      relationField('commune_id', 'Commune', communeRelation, true, 'Affectation'),
      field('grade', 'Grade', 'text', true, undefined, undefined, 'Affectation'),
      field('date_prise_fonction', 'Date prise fonction', 'date', false, undefined, undefined, 'Affectation'),
      field('formation_nasla', 'Formation NASLA validee', 'checkbox', false, undefined, undefined, 'Affectation'),
      field('telephone', 'Telephone', 'text', false, undefined, undefined, 'Contact'),
      field('email', 'Email', 'email', false, undefined, undefined, 'Contact'),
      relationField('user_id', 'Compte utilisateur associe', userRelation, false, 'Contact'),
    ],
    actions: [
      sensitiveAction('Suspendre', (row) => `/api/v1/agents/${row['id']}/suspend`, 'Suspendre cet agent ?'),
      sensitiveAction('Reactiver', (row) => `/api/v1/agents/${row['id']}/reactivate`, 'Reactiver cet agent ?'),
      sensitiveAction('Retraite', (row) => `/api/v1/agents/${row['id']}/retire`, 'Mettre cet agent a la retraite ?'),
    ],
  },
  zones: {
    key: 'zones',
    title: 'Zones',
    description: 'Quartiers, secteurs, marches et zones sensibles par commune.',
    endpoint: '/api/v1/zones',
    columns: ['nom', 'type_zone', 'commune_id', 'parent_id', 'active'],
    detailFields: ['nom', 'type_zone', 'commune_id', 'parent_id', 'active'],
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [relationFilter('commune_id', 'Commune', communeRelation), activeFilter()],
    createFields: [
      relationField('commune_id', 'Commune', communeRelation),
      field('nom', 'Nom de zone', 'text', true),
      selectField('type_zone', 'Type de zone', true, [
        option('QUARTIER', 'Quartier'),
        option('BLOC', 'Bloc'),
        option('SECTEUR', 'Secteur'),
        option('MARCHE', 'Marche'),
        option('ZONE_SENSIBLE', 'Zone sensible'),
      ]),
      relationField('parent_id', 'Zone parente', zoneRelation, false),
      field('active', 'Zone active', 'checkbox'),
    ],
  },
  'referentiel-categories': {
    key: 'referentiel-categories',
    title: 'Categories',
    description: 'Premier niveau du referentiel communal.',
    endpoint: '/api/v1/referentiel/categories',
    columns: ['nom', 'commune_id', 'description', 'active'],
    detailFields: ['nom', 'commune_id', 'description', 'active'],
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [relationFilter('commune_id', 'Commune', communeRelation), activeFilter()],
    createFields: [
      relationField('commune_id', 'Commune', communeRelation),
      field('nom', 'Nom categorie', 'text', true),
      field('description', 'Description', 'textarea'),
      field('active', 'Categorie active', 'checkbox'),
    ],
  },
  'referentiel-types': {
    key: 'referentiel-types',
    title: 'Types intervention',
    description: 'Deuxieme niveau du referentiel communal.',
    endpoint: '/api/v1/referentiel/types',
    columns: ['nom', 'category_id', 'commune_id', 'description', 'active'],
    detailFields: ['nom', 'category_id', 'commune_id', 'description', 'active'],
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [
      relationFilter('commune_id', 'Commune', communeRelation),
      relationFilter('category_id', 'Categorie', categoryRelation),
      activeFilter(),
    ],
    createFields: [
      relationField('commune_id', 'Commune', communeRelation, true, 'Cascade referentiel'),
      relationField('category_id', 'Categorie', categoryRelation, true, 'Cascade referentiel'),
      field('nom', 'Nom type', 'text', true, undefined, undefined, 'Definition'),
      field('description', 'Description', 'textarea', false, undefined, undefined, 'Definition'),
      field('active', 'Type actif', 'checkbox', false, undefined, undefined, 'Definition'),
    ],
  },
  'referentiel-interventions': {
    key: 'referentiel-interventions',
    title: 'Interventions',
    description: 'Montants, delais, penalites et references de deliberation.',
    endpoint: '/api/v1/referentiel/interventions',
    columns: ['nom', 'type_id', 'sujet_paiement', 'montant_fcfa', 'delai_paiement_jours', 'active'],
    secondaryColumns: ['reference_deliberation'],
    detailFields: [
      'nom',
      'commune_id',
      'category_id',
      'type_id',
      'description',
      'sujet_paiement',
      'montant_fcfa',
      'delai_paiement_jours',
      'taux_penalite_basis_points',
      'reference_deliberation',
      'active',
    ],
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [
      relationFilter('commune_id', 'Commune', communeRelation),
      relationFilter('category_id', 'Categorie', categoryRelation),
      relationFilter('type_id', 'Type', typeRelation),
      activeFilter(),
    ],
    createFields: [
      relationField('commune_id', 'Commune', communeRelation, true, 'Cascade referentiel'),
      relationField('category_id', 'Categorie', categoryRelation, false, 'Cascade referentiel'),
      relationField('type_id', 'Type intervention', typeRelation, true, 'Cascade referentiel'),
      field('nom', 'Nom intervention', 'text', true, undefined, undefined, 'Regle financiere'),
      field('description', 'Description', 'textarea', false, undefined, undefined, 'Regle financiere'),
      field('sujet_paiement', 'Sujet a paiement', 'checkbox', false, undefined, 'Desactive pour un avertissement ou une intervention non payante.', 'Regle financiere'),
      field('montant_fcfa', 'Montant FCFA', 'money', false, undefined, 'Montant officiel issu de la deliberation.', 'Regle financiere'),
      field('delai_paiement_jours', 'Delai paiement', 'number', false, '30', 'Nombre de jours avant penalite.', 'Regle financiere'),
      field('taux_penalite_basis_points', 'Penalite', 'number', false, '500', 'Basis points: 500 = 5%.', 'Regle financiere'),
      field('reference_deliberation', 'Reference deliberation', 'text', false, undefined, undefined, 'Regle financiere'),
      field('active', 'Intervention active', 'checkbox', false, undefined, undefined, 'Regle financiere'),
    ],
  },
  pvs: {
    key: 'pvs',
    title: 'Proces-verbaux',
    description: 'Creation, suivi, QR code et impression des PV.',
    endpoint: '/api/v1/pvs',
    columns: ['pv_number', 'status', 'amount_initial_fcfa', 'vehicle_plate', 'created_at'],
    secondaryColumns: ['intervention_id', 'zone_id', 'verbalized_name'],
    detailFields: [
      'pv_number',
      'status',
      'intervention_id',
      'amount_initial_fcfa',
      'vehicle_plate',
      'verbalized_name',
      'verbalized_identifier',
      'zone_id',
      'location_description',
      'gps_latitude',
      'gps_longitude',
      'notes_internes',
      'created_at',
    ],
    labels: commonLabels(),
    createRoles: ['APM_AGENT'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE', 'SUPERVISEUR'],
    filters: [statusFilter(statusOptions['pvs']), relationFilter('agent_id', 'Agent', {
      endpoint: '/api/v1/agents',
      labelKey: 'full_name',
      metaKey: 'matricule',
      statusKey: 'status',
    })],
    createFields: [
      relationField('intervention_id', 'Intervention', interventionRelation, true, '1. Intervention'),
      relationField('zone_id', 'Zone', zoneRelation, false, '1. Intervention'),
      field('verbalized_name', 'Nom verbalise', 'text', false, undefined, undefined, '2. Verbalise'),
      field('verbalized_identifier', 'Identifiant verbalise', 'text', false, 'CNI, NIU ou autre reference', undefined, '2. Verbalise'),
      field('vehicle_plate', 'Plaque vehicule', 'text', false, undefined, undefined, '2. Verbalise'),
      field('location_description', 'Lieu', 'textarea', false, undefined, undefined, '3. Localisation'),
      field('gps_latitude', 'Latitude', 'number', false, undefined, undefined, '3. Localisation'),
      field('gps_longitude', 'Longitude', 'number', false, undefined, undefined, '3. Localisation'),
      field('notes_internes', 'Notes internes', 'textarea', false, undefined, undefined, '4. Recapitulatif'),
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
    ],
  },
  signalements: {
    key: 'signalements',
    title: 'Signalements',
    description: 'Signalements citoyens, priorisation et suivi administratif.',
    endpoint: '/api/v1/signalements',
    columns: ['signalement_number', 'type_incident', 'location_description', 'status', 'created_at'],
    secondaryColumns: ['description'],
    detailFields: ['signalement_number', 'type_incident', 'location_description', 'description', 'status', 'contact_anonyme', 'created_at', 'updated_at'],
    labels: commonLabels(),
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [statusFilter(statusOptions['signalements']), relationFilter('commune_id', 'Commune', communeRelation)],
  },
  patrouilles: {
    key: 'patrouilles',
    title: 'Patrouilles',
    description: 'Planification, demarrage et cloture des patrouilles.',
    endpoint: '/api/v1/patrouilles',
    columns: ['nom', 'status', 'zone_id', 'date_debut', 'date_fin'],
    secondaryColumns: ['description'],
    detailFields: ['nom', 'description', 'commune_id', 'zone_id', 'status', 'date_debut', 'date_fin'],
    labels: commonLabels(),
    createRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    mutateRoles: ['SUPER_ADMIN', 'ADMIN_COMMUNE'],
    filters: [statusFilter(statusOptions['patrouilles']), relationFilter('zone_id', 'Zone', zoneRelation)],
    createFields: [
      relationField('commune_id', 'Commune', communeRelation),
      relationField('zone_id', 'Zone', zoneRelation, false),
      field('nom', 'Nom patrouille', 'text', true),
      field('description', 'Description', 'textarea'),
    ],
    actions: [
      sensitiveAction('Demarrer', (row) => `/api/v1/patrouilles/${row['id']}/start`, 'Demarrer cette patrouille ?'),
      sensitiveAction('Cloturer', (row) => `/api/v1/patrouilles/${row['id']}/end`, 'Cloturer cette patrouille ?'),
    ],
  },
  'audit-logs': {
    key: 'audit-logs',
    title: 'Audit logs',
    description: 'Journal des actions sensibles.',
    endpoint: '/api/v1/audit-logs',
    columns: ['action', 'entity_type', 'entity_id', 'user_id', 'created_at'],
    detailFields: ['action', 'entity_type', 'entity_id', 'user_id', 'commune_id', 'ip_address', 'user_agent', 'created_at'],
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
): ResourceField {
  return { key, label, type: 'relation', required, relation, section };
}

function selectField(
  key: string,
  label: string,
  required: boolean,
  options: SelectOption[],
  help?: string,
): ResourceField {
  return { key, label, type: 'select', required, options, help };
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
    label: 'Etat',
    type: 'active',
    options: [
      option(true, 'Actif'),
      option(false, 'Inactif'),
    ],
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
      const target = String(row['full_name'] ?? row['nom'] ?? row['pv_number'] ?? row['id'] ?? 'cet element');
      return `Cette action modifie un statut sensible pour ${target}. Elle sera journalisee.`;
    },
  };
}

function commonLabels(): Record<string, string> {
  return {
    id: 'ID',
    code: 'Code',
    nom: 'Nom',
    full_name: 'Nom complet',
    email: 'Email',
    roles: 'Roles',
    commune_id: 'Commune',
    active: 'Actif',
    status: 'Statut',
    matricule: 'Matricule',
    grade: 'Grade',
    date_prise_fonction: 'Prise de fonction',
    formation_nasla: 'NASLA',
    region: 'Region',
    departement: 'Departement',
    adresse: 'Adresse',
    telephone: 'Telephone',
    theme_color: 'Couleur theme',
    type_zone: 'Type',
    parent_id: 'Zone parente',
    description: 'Description',
    category_id: 'Categorie',
    type_id: 'Type',
    intervention_id: 'Intervention',
    sujet_paiement: 'Payant',
    montant_fcfa: 'Montant FCFA',
    amount_initial_fcfa: 'Montant FCFA',
    amount_paid_fcfa: 'Montant encaisse',
    delai_paiement_jours: 'Delai',
    taux_penalite_basis_points: 'Penalite',
    reference_deliberation: 'Deliberation',
    pv_number: 'Numero PV',
    verbalized_name: 'Verbalise',
    verbalized_identifier: 'Identifiant',
    vehicle_plate: 'Plaque',
    gps_latitude: 'Latitude',
    gps_longitude: 'Longitude',
    notes_internes: 'Notes internes',
    created_at: 'Cree le',
    updated_at: 'Mis a jour',
    date_debut: 'Debut',
    date_fin: 'Fin',
    zone_id: 'Zone',
    signalement_number: 'Numero',
    type_incident: 'Incident',
    location_description: 'Lieu',
    contact_anonyme: 'Anonyme',
    entity_type: 'Entite',
    entity_id: 'Entite ID',
    user_id: 'Utilisateur',
    action: 'Action',
    ip_address: 'IP',
    user_agent: 'Navigateur',
  };
}
