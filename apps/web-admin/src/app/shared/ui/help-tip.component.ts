import { Component, HostListener, input, signal } from '@angular/core';

/**
 * Pictogramme d'aide « ? » accessible (remarques 5 & 12) : un bouton focalisable
 * qui ouvre une bulle d'explication courte. Réutilisable à côté d'un titre de
 * fonctionnalité ou d'un lien de navigation.
 */
@Component({
  selector: 'app-help-tip',
  standalone: true,
  template: `
    <span class="relative inline-flex align-middle">
      <button
        type="button"
        class="grid h-5 w-5 place-items-center rounded-full border border-[var(--line-subtle)] bg-white text-xs font-black leading-none text-[var(--text-muted)] hover:text-[var(--text-strong)]"
        [attr.aria-label]="label()"
        [attr.aria-expanded]="open()"
        (click)="toggle($event)"
      >
        ?
      </button>
      @if (open()) {
        <span
          class="absolute left-1/2 top-7 z-30 w-64 -translate-x-1/2 rounded-md border border-[var(--line-subtle)] bg-white p-3 text-left text-xs font-semibold leading-relaxed text-[var(--text-strong)] shadow-[var(--shadow-soft)]"
          role="tooltip"
        >
          {{ text() }}
        </span>
      }
    </span>
  `,
})
export class HelpTipComponent {
  /** Texte d'explication affiché dans la bulle. */
  readonly text = input.required<string>();
  /** Libellé accessible du bouton (défaut générique). */
  readonly label = input<string>('Aide');

  protected readonly open = signal(false);

  protected toggle(event: MouseEvent): void {
    event.stopPropagation();
    this.open.update((value) => !value);
  }

  @HostListener('document:click')
  protected close(): void {
    this.open.set(false);
  }

  @HostListener('document:keydown.escape')
  protected dismiss(): void {
    this.open.set(false);
  }
}
