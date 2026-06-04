import { HttpClient, HttpParams } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';

import { apiBaseUrl } from '../config/runtime-config';
import { Paginated } from '../../shared/api-types';

type QueryValue = string | number | boolean | null | undefined;

@Injectable({ providedIn: 'root' })
export class ApiService {
  private readonly http = inject(HttpClient);
  private readonly baseUrl = apiBaseUrl();

  get<T>(path: string, query?: Record<string, QueryValue>) {
    return this.http.get<T>(this.url(path), { params: this.params(query) });
  }

  page<T>(path: string, query?: Record<string, QueryValue>) {
    return this.get<Paginated<T>>(path, query);
  }

  post<T>(path: string, body: unknown, query?: Record<string, QueryValue>) {
    return this.http.post<T>(this.url(path), body, { params: this.params(query) });
  }

  postText<T>(path: string, body: string, query?: Record<string, QueryValue>) {
    return this.http.post<T>(this.url(path), body, {
      params: this.params(query),
      headers: { 'content-type': 'text/csv; charset=utf-8' },
    });
  }

  patch<T>(path: string, body: unknown) {
    return this.http.patch<T>(this.url(path), body);
  }

  delete<T>(path: string) {
    return this.http.delete<T>(this.url(path));
  }

  download(path: string, query?: Record<string, QueryValue>) {
    return this.http.get(this.url(path), {
      params: this.params(query),
      responseType: 'blob',
    });
  }

  openDownload(path: string, filename: string, query?: Record<string, QueryValue>) {
    this.download(path, query).subscribe((blob) => {
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = filename;
      link.click();
      URL.revokeObjectURL(url);
    });
  }

  private url(path: string): string {
    const normalized = path.startsWith('/') ? path : `/${path}`;
    return `${this.baseUrl}${normalized}`;
  }

  private params(query?: Record<string, QueryValue>): HttpParams {
    let params = new HttpParams();
    for (const [key, value] of Object.entries(query ?? {})) {
      if (value !== undefined && value !== null && value !== '') {
        params = params.set(key, String(value));
      }
    }
    return params;
  }
}
