/**
 * Extract a human-readable message from any thrown value.
 *
 * Tauri commands reject with the serialized AppError enum, e.g.:
 *   {"Service": "wavs-mcp binary not found"}
 *   {"WavsNotRunning": null}
 * Plain strings and Error objects are handled too.
 */
export function errorMessage(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  if (typeof e === 'object' && e !== null) {
    const values = Object.values(e as Record<string, unknown>);
    if (values.length > 0 && typeof values[0] === 'string') return values[0];
    return JSON.stringify(e);
  }
  return String(e);
}
