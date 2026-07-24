export function shouldEnterSubmit(
  completionStatus: string | null,
  selectedIndex: number | null,
): boolean {
  if (completionStatus === 'active' && selectedIndex !== null && selectedIndex >= 0) return false
  return true
}

export function shouldEscapeSubmit(
  completionStatus: string | null,
  selectedIndex: number | null,
): boolean {
  if (completionStatus === null) return true
  if (completionStatus === 'active' && selectedIndex !== null && selectedIndex >= 0) return false
  return true
}
