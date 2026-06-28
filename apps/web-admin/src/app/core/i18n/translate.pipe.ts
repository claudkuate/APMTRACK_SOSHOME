import { Pipe, PipeTransform, inject } from '@angular/core';

import { I18nService } from './i18n.service';

/**
 * Pipe de traduction `{{ 'ma.cle' | t }}`. Impur volontairement : il relit la
 * langue active du {@link I18nService} à chaque cycle de détection afin que tous
 * les libellés se rafraîchissent lors d'un changement de langue.
 */
@Pipe({ name: 't', standalone: true, pure: false })
export class TranslatePipe implements PipeTransform {
  private readonly i18n = inject(I18nService);

  transform(key: string, params?: Record<string, string | number>): string {
    return this.i18n.t(key, params);
  }
}
