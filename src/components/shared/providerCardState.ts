export function restoreProviderRadioState(
  radio: Pick<HTMLInputElement, 'checked'>,
  active: boolean,
) {
  radio.checked = active
}
