export type Theme = 'light' | 'dark' | 'system'

export const applyTheme = (value: Theme) => {
  const root = document.documentElement
  root.classList.remove('theme-dark', 'theme-light')

  if (value === 'dark') {
    root.classList.add('theme-dark')
  } else if (value === 'light') {
    root.classList.add('theme-light')
  } else {
    const isDark = window.matchMedia('(prefers-color-scheme: dark)').matches
    root.classList.add(isDark ? 'theme-dark' : 'theme-light')
  }
}