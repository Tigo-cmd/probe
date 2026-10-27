import { describe, expect, it } from 'vitest';
import { warrantyLink } from './warranty.js';

const id = (vendor, serial) => ({ vendor: { value: vendor }, serial: { value: serial } });

describe('warrantyLink', () => {
  it('puts a Dell service tag in the link', () => {
    const w = warrantyLink(id('Dell Inc.', 'ABC1234'));
    expect(w.maker).toBe('Dell');
    expect(w.url).toContain('/servicetag/ABC1234/');
    expect(w.prefilled).toBe(true);
  });
  it('opens the lookup page for Lenovo, HP and Apple', () => {
    expect(warrantyLink(id('LENOVO', 'PF0ABCDE')).url).toBe('https://pcsupport.lenovo.com/warrantylookup');
    expect(warrantyLink(id('HP', '5CG1234')).maker).toBe('HP');
    expect(warrantyLink(id('Hewlett-Packard', '5CG1234')).maker).toBe('HP');
    expect(warrantyLink(id('Apple Inc.', 'C02XYZ')).maker).toBe('Apple');
  });
  it('gives nothing without a real serial or for other makers', () => {
    expect(warrantyLink(id('Dell Inc.', ''))).toBeNull();
    expect(warrantyLink(id('Dell Inc.', 'To Be Filled By O.E.M.'))).toBeNull();
    expect(warrantyLink(id('ASUSTeK COMPUTER INC.', 'X1'))).toBeNull();
    expect(warrantyLink(id('Shopping Mall', 'X1'))).toBeNull();
  });
});
