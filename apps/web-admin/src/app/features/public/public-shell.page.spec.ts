import { TestBed } from '@angular/core/testing';
import { provideRouter } from '@angular/router';

import { PublicShellPage } from './public-shell.page';

describe('PublicShellPage', () => {
  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [PublicShellPage],
      providers: [provideRouter([])],
    }).compileComponents();
  });

  it('uses the Reunification Monument as the public background', () => {
    const fixture = TestBed.createComponent(PublicShellPage);
    fixture.detectChanges();

    const element = fixture.nativeElement as HTMLElement;
    const backgrounds = element.querySelectorAll('main > img[aria-hidden="true"]');
    const background = backgrounds.item(0);

    expect(backgrounds.length).toBe(1);
    expect(background?.getAttribute('src')).toBe('/yaounde-reunification-login-hero.png');
  });
});
