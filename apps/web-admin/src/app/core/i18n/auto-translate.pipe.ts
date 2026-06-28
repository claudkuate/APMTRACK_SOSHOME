import { Pipe, PipeTransform, inject } from '@angular/core';

import { I18nService } from './i18n.service';

/**
 * Pipe `{{ 'Libellé FR' | auto }}` : traduit un littéral français du back-office
 * vers l'anglais (cf. {@link I18nService.auto}). Impur pour se rafraîchir au
 * changement de langue.
 */
@Pipe({ name: 'auto', standalone: true, pure: false })
export class AutoTranslatePipe implements PipeTransform {
  private readonly i18n = inject(I18nService);

  transform(value: string | null | undefined): string {
    return this.i18n.auto(value);
  }
}
