import { HttpClient, HttpParams } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { forkJoin, of } from 'rxjs';
import { map, switchMap } from 'rxjs/operators';

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

  /**
   * Charge TOUTES les pages d'une ressource et renvoie la liste concaténée.
   *
   * Le serveur plafonne `page_size` à 100 (`pagination.rs`) : on ne peut donc pas régler
   * la troncature en demandant une page plus grande. Avec ~360 communes au niveau
   * national, un simple `page_size: 100` amputait silencieusement le sélecteur de
   * commune, la page Exports et les listes de relations.
   */
  pageAll<T>(path: string, query?: Record<string, QueryValue>, maxPages = 20) {
    const pageSize = 100;
    return this.page<T>(path, { ...query, page: 1, page_size: pageSize }).pipe(
      switchMap((first) => {
        const pages = Math.min(Math.ceil(first.total / pageSize), maxPages);
        if (pages <= 1) {
          return of([first.items]);
        }
        const rest = Array.from({ length: pages - 1 }, (_, index) =>
          this.page<T>(path, { ...query, page: index + 2, page_size: pageSize }),
        );
        return forkJoin(rest).pipe(
          map((responses) => [first.items, ...responses.map((response) => response.items)]),
        );
      }),
      map((chunks) => chunks.flat()),
    );
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

  openDownload(
    path: string,
    filename: string,
    query?: Record<string, QueryValue>,
    onError?: (error: unknown) => void,
  ) {
    this.download(path, query).subscribe({
      next: (blob) => {
        const url = URL.createObjectURL(blob);
        const link = document.createElement('a');
        link.href = url;
        link.download = filename;
        link.click();
        URL.revokeObjectURL(url);
      },
      // Sans handler, un téléchargement en échec (403/500/hors-ligne) était
      // totalement silencieux : ni fichier, ni message.
      error: (error: unknown) => onError?.(error),
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
