// Where to check a computer's warranty. Makers only offer warranty APIs to
// approved business partners, so probe opens the maker's own page and
// copies the serial number for pasting; Dell takes it in the link itself.

const MAKERS = [
  {
    match: /\bdell\b/i,
    name: 'Dell',
    url: (s) => `https://www.dell.com/support/home/en-us/product-support/servicetag/${encodeURIComponent(s)}/overview`,
    prefilled: true,
  },
  { match: /\blenovo\b/i, name: 'Lenovo', url: () => 'https://pcsupport.lenovo.com/warrantylookup', prefilled: false },
  { match: /\b(hp|hewlett)/i, name: 'HP', url: () => 'https://support.hp.com/us-en/check-warranty', prefilled: false },
  { match: /\bapple\b/i, name: 'Apple', url: () => 'https://checkcoverage.apple.com', prefilled: false },
];

/**
 * The warranty page for a scan's identity, or null when the maker is not
 * one with a public warranty check or the serial was not read.
 * @returns {{ maker: string, url: string, serial: string, prefilled: boolean } | null}
 */
export function warrantyLink(identity) {
  const vendor = identity?.vendor?.value ?? '';
  const serial = (identity?.serial?.value ?? '').trim();
  if (!serial || /^(0+|to be filled.*|default string|system serial number|n\/a|none)$/i.test(serial)) return null;
  const maker = MAKERS.find((m) => m.match.test(vendor));
  if (!maker) return null;
  return { maker: maker.name, url: maker.url(serial), serial, prefilled: maker.prefilled };
}
