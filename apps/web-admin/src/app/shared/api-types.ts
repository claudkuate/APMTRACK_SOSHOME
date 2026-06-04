export type RoleCode =
  | 'SUPER_ADMIN'
  | 'ADMIN_COMMUNE'
  | 'APM_AGENT'
  | 'SUPERVISEUR'
  | 'RECEVEUR';

export interface Paginated<T> {
  items: T[];
  page: number;
  page_size: number;
  total: number;
}

export interface CurrentUser {
  id: string;
  email: string;
  full_name: string;
  commune_id: string | null;
  roles: RoleCode[];
  active: boolean;
}

export interface TokenResponse {
  access_token: string;
  refresh_token?: string;
  token_type: 'Bearer';
  expires_in_seconds: number;
  user: CurrentUser;
}

export interface ApiErrorEnvelope {
  error?: {
    code: string;
    message: string;
    details?: unknown;
  };
}

export interface ApiHealth {
  status: string;
  service: string;
  environment: string;
  version: string;
}

export interface DashboardSummary {
  pvs: Record<string, number>;
  payments: Record<string, number>;
  agents: Record<string, number>;
  signalements: Record<string, number>;
  patrouilles: Record<string, number>;
  commune_id: string | null;
}

export interface LookupOption {
  id: string;
  label: string;
  meta?: string;
  status?: string;
}

export interface SearchResult {
  module: string;
  id: string;
  title: string;
  detail: string;
  status?: string;
  route: string;
}
