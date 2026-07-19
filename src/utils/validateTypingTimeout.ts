export function normalizeTypingTimeout(raw: unknown): number | null {
  if (raw === '' || raw === null || raw === undefined) return null
  const n = Number(raw)
  if (!Number.isFinite(n)) return null
  if (!Number.isInteger(n)) return null
  return Math.max(200, Math.min(5000, n))
}
