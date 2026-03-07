import { useTheme } from '../lib/theme'

export function ThemeToggle() {
  const { theme, toggleTheme } = useTheme()

  return (
    <button
      className="btn-logout"
      onClick={toggleTheme}
      title={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}
      aria-label={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}
    >
      {theme === 'dark' ? '$ light' : '$ dark'}
    </button>
  )
}
