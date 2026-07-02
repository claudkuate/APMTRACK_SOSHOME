import { csvCell, toCsv } from './csv';

describe('csvCell', () => {
  it('met la valeur entre guillemets et double les guillemets internes', () => {
    expect(csvCell('Douala 1er')).toBe('"Douala 1er"');
    expect(csvCell('dit "Le Boss"')).toBe('"dit ""Le Boss"""');
  });

  it("neutralise les préfixes de formule tableur (= + - @ et contrôles)", () => {
    expect(csvCell('=HYPERLINK("http://evil")')).toBe('"\'=HYPERLINK(""http://evil"")"');
    expect(csvCell('+237690000000')).toBe('"\'+237690000000"');
    expect(csvCell('-1000')).toBe('"\'-1000"');
    expect(csvCell('@import')).toBe('"\'@import"');
  });

  it('laisse intactes les valeurs ordinaires', () => {
    expect(csvCell('PV-YDE1-2026-000001')).toBe('"PV-YDE1-2026-000001"');
    expect(csvCell('10 000 FCFA')).toBe('"10 000 FCFA"');
  });
});

describe('toCsv', () => {
  it('assemble en-tête et lignes avec cellules protégées', () => {
    const csv = toCsv([
      ['Nom', 'Montant'],
      ['=cmd', '5000'],
    ]);
    expect(csv).toBe('"Nom","Montant"\n"\'=cmd","5000"');
  });
});
