export type Theme = 'dark' | 'light' | 'brand'

export const DEFAULT_THEME: Theme = 'light'
export const THEME_ORDER: Theme[] = ['light', 'brand', 'dark']

export const resolveTheme = (saved: string | null): Theme =>
  saved === 'dark' || saved === 'light' || saved === 'brand' ? saved : DEFAULT_THEME

export const nextTheme = (current: Theme): Theme =>
  THEME_ORDER[(THEME_ORDER.indexOf(current) + 1) % THEME_ORDER.length]
