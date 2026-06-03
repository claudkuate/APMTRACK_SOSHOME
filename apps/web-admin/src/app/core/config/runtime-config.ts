export interface RuntimeConfig {
  apiUrl?: string;
  environment?: string;
}

declare global {
  interface Window {
    __APMTRACK_CONFIG__?: RuntimeConfig;
  }
}

export function apiBaseUrl(): string {
  return normalizeUrl(window.__APMTRACK_CONFIG__?.apiUrl ?? 'http://localhost:8080');
}

export function runtimeEnvironment(): string {
  return window.__APMTRACK_CONFIG__?.environment ?? 'development';
}

function normalizeUrl(url: string): string {
  return url.endsWith('/') ? url.slice(0, -1) : url;
}

