import { ref, provide, inject } from 'vue'

export const CAPTURE_KEY = Symbol('keybind-active-id')

export type ActiveIdRef = ReturnType<typeof ref<symbol | null>>

export function provideKeybindContext() {
  const activeId = ref<symbol | null>(null)
  provide(CAPTURE_KEY, activeId)
  return activeId
}

export function useKeybindActiveId(): ActiveIdRef {
  return inject<ActiveIdRef>(CAPTURE_KEY, ref(null))
}

export const MODIFIER_CODES = new Set([
  'ControlLeft', 'ControlRight',
  'AltLeft',     'AltRight',
  'ShiftLeft',   'ShiftRight',
])

export const IGNORED_CODES = new Set([
  'CapsLock', 'NumLock', 'ScrollLock', 'ContextMenu', 'AltGraph',
])

const REGULAR_CODES = new Set([
  'Space', 'Enter', 'Tab', 'Backspace', 'Delete', 'Insert',
  'Home', 'End', 'PageUp', 'PageDown', 'Escape', 'PrintScreen', 'Pause',
  'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight',
  'Minus', 'Equal', 'BracketLeft', 'BracketRight', 'Backslash',
  'Semicolon', 'Quote', 'Comma', 'Period', 'Slash', 'Backquote',
])

export function isRegularCode(code: string): boolean {
  return (
    /^Key[A-Z]$/.test(code)  ||
    /^Digit\d$/.test(code)   ||
    /^F\d{1,2}$/.test(code)  ||
    /^Numpad/.test(code)     ||
    REGULAR_CODES.has(code)
  )
}

export function modifierParts(e: KeyboardEvent): string[] {
  const parts: string[] = []
  if (e.ctrlKey)  parts.push('Ctrl')
  if (e.altKey)   parts.push('Alt')
  if (e.shiftKey) parts.push('Shift')
  if (e.metaKey)  parts.push('Super')
  return parts
}

export function eventToShortcutString(e: KeyboardEvent): string | null {
  if (MODIFIER_CODES.has(e.code) || IGNORED_CODES.has(e.code)) return null
  if (!isRegularCode(e.code)) return null
  return [...modifierParts(e), e.code].join('+')
}

export function modifierOnlyShortcutFromKeyup(e: KeyboardEvent): string | null {
  const relCtrl  = e.code === 'ControlLeft' || e.code === 'ControlRight'
  const relAlt   = e.code === 'AltLeft'     || e.code === 'AltRight'
  const relShift = e.code === 'ShiftLeft'   || e.code === 'ShiftRight'

  const parts: string[] = []
  if (e.ctrlKey  || relCtrl)  parts.push('Ctrl')
  if (e.altKey   || relAlt)   parts.push('Alt')
  if (e.shiftKey || relShift) parts.push('Shift')
  return parts.length ? parts.join('+') : null
}

export function formatShortcutDisplay(shortcut: string): string {
  return shortcut.split('+').map(part => {
    if (/^Key[A-Z]$/.test(part))   return part.slice(3)
    if (/^Digit\d$/.test(part))    return part.slice(5)
    if (/^Numpad(.+)$/.test(part)) return 'Num' + part.slice(6)
    return part
  }).join('+')
}