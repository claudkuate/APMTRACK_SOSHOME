import { HttpClient } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';

import { apiBaseUrl } from '../config/runtime-config';

export interface ApiHealth {
  status: string;
  service: string;
  environment: string;
  version: string;
}

export interface DbHealth {
  status: string;
  service: string;
  environment: string;
  database: string;
}

@Injectable({ providedIn: 'root' })
export class HealthService {
  private readonly http = inject(HttpClient);
  private readonly baseUrl = apiBaseUrl();

  api() {
    return this.http.get<ApiHealth>(`${this.baseUrl}/health`);
  }

  database() {
    return this.http.get<DbHealth>(`${this.baseUrl}/health/db`);
  }
}

