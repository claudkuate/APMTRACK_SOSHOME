import { Component, OnInit, computed, inject, signal } from '@angular/core';

import { HealthService } from '../../core/services/health.service';
import { apiBaseUrl, runtimeEnvironment } from '../../core/config/runtime-config';

type CheckState = 'loading' | 'ok' | 'error';

interface StatusItem {
  label: string;
  state: CheckState;
  detail: string;
}

@Component({
  selector: 'app-status',
  templateUrl: './status.html',
  styleUrl: './status.scss',
})
export class Status implements OnInit {
  private readonly health = inject(HealthService);

  protected readonly apiUrl = apiBaseUrl();
  protected readonly environment = runtimeEnvironment();
  protected readonly api = signal<StatusItem>({
    label: 'API Rust',
    state: 'loading',
    detail: 'Verification en cours',
  });
  protected readonly database = signal<StatusItem>({
    label: 'PostgreSQL',
    state: 'loading',
    detail: 'Verification en cours',
  });
  protected readonly summary = computed(() => {
    const checks = [this.api(), this.database()];
    return checks.every((item) => item.state === 'ok') ? 'operationnel' : 'a verifier';
  });

  ngOnInit(): void {
    this.refresh();
  }

  protected refresh(): void {
    this.api.set({ label: 'API Rust', state: 'loading', detail: 'Verification en cours' });
    this.database.set({ label: 'PostgreSQL', state: 'loading', detail: 'Verification en cours' });

    this.health.api().subscribe({
      next: (response) => {
        this.api.set({
          label: 'API Rust',
          state: response.status === 'ok' ? 'ok' : 'error',
          detail: `${response.service} ${response.version} - ${response.environment}`,
        });
      },
      error: () => {
        this.api.set({
          label: 'API Rust',
          state: 'error',
          detail: 'Endpoint /health indisponible',
        });
      },
    });

    this.health.database().subscribe({
      next: (response) => {
        this.database.set({
          label: 'PostgreSQL',
          state: response.status === 'ok' ? 'ok' : 'error',
          detail: response.database,
        });
      },
      error: () => {
        this.database.set({
          label: 'PostgreSQL',
          state: 'error',
          detail: 'Endpoint /health/db indisponible',
        });
      },
    });
  }
}

