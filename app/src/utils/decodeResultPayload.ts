export type DecodeResult =
  | { kind: 'json'; display: string; truncated: boolean }
  | { kind: 'text'; display: string; truncated: boolean }
  | { kind: 'hex'; display: string; truncated: boolean };

export function decodeResultPayload(resultPayload: string | null | undefined): DecodeResult {
  // Guard: null, undefined, or empty string
  if (!resultPayload) {
    return { kind: 'hex', display: '\u2014', truncated: false };
  }

  // Step 1 — hex to bytes
  const clean = resultPayload.replace(/^0x/i, '');
  const bytes = new Uint8Array(Math.floor(clean.length / 2));
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(clean.substring(i * 2, i * 2 + 2), 16);
  }

  // Step 2 — attempt UTF-8
  let text: string;
  try {
    text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch {
    // Step 4 — hex fallback
    const hexStr = clean.slice(0, 40);
    const truncated = clean.length > 40;
    return {
      kind: 'hex',
      display: truncated ? `${hexStr}\u2026 (${bytes.length} bytes)` : hexStr,
      truncated,
    };
  }

  // Step 3 — attempt JSON parse
  try {
    const parsed = JSON.parse(text);
    const pretty = JSON.stringify(parsed, null, 2);
    return { kind: 'json', display: pretty, truncated: false };
  } catch {
    return { kind: 'text', display: text, truncated: false };
  }
}
