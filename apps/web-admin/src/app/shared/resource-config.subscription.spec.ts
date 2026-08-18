import { resolveSelectOptionValue, resourceConfigs } from './resource-config';

describe('configuration des abonnements de communes', () => {
  const communes = resourceConfigs['communes'];

  it("n'expose active ni à la création ni dans le PATCH générique", () => {
    expect(communes.createFields?.some((field) => field.key === 'active')).toBe(false);
    expect(communes.patchFields?.some((field) => field.key === 'active')).toBe(false);
    expect(
      communes.createFields?.some((field) => field.key.startsWith('subscription_')),
    ).toBe(false);
    expect(
      communes.patchFields?.some((field) => field.key.startsWith('subscription_')),
    ).toBe(false);
  });

  it('propose uniquement une suspension explicite aux super-administrateurs', () => {
    const action = communes.actions?.find((candidate) => candidate.label === 'Suspendre la mairie');

    expect(action).toBeTruthy();
    expect(action?.roles).toEqual(['SUPER_ADMIN']);
    expect(action?.method).toBe('patch');
    expect(action?.statusKey).toBe('active');
    expect(action?.statusOptions).toEqual([{ value: false, label: 'Suspendre l’accès' }]);
    expect(action?.visibleWhen?.({ active: true })).toBe(true);
    expect(action?.visibleWhen?.({ active: false })).toBe(false);
  });

  it('préserve le booléen false dans le payload issu du sélecteur de suspension', () => {
    expect(resolveSelectOptionValue([{ value: false, label: 'Suspendre' }], 'false')).toBe(false);
  });
});
