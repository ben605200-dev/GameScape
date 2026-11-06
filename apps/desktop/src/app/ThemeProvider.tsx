import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { DEFAULT_THEME, isThemeName, type ThemeName } from "@gamescape/ui";
import { getSettings, saveSettings } from "../lib/ipc";

interface ThemeContextValue {
  theme: ThemeName;
  setTheme: (theme: ThemeName) => void;
}

const ThemeContext = createContext<ThemeContextValue>({
  theme: DEFAULT_THEME,
  setTheme: () => undefined,
});

export function useTheme(): ThemeContextValue {
  return useContext(ThemeContext);
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeName>(DEFAULT_THEME);

  // Load the persisted preference (Rust settings store in the app,
  // localStorage in plain-browser preview).
  useEffect(() => {
    let cancelled = false;
    void getSettings().then((settings) => {
      if (!cancelled && isThemeName(settings.theme)) {
        setThemeState(settings.theme);
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  const setTheme = (next: ThemeName) => {
    setThemeState(next);
    void saveSettings({ theme: next });
  };

  return (
    <ThemeContext.Provider value={{ theme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}
