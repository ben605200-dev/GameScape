import { Moon, Sun } from "lucide-react";
import { useLocation } from "react-router-dom";
import { NAV_ITEMS } from "../../app/nav";
import { useTheme } from "../../app/ThemeProvider";

export function TopBar() {
  const location = useLocation();
  const { theme, setTheme } = useTheme();

  const current = NAV_ITEMS.find((item) =>
    item.path === "/"
      ? location.pathname === "/"
      : location.pathname.startsWith(item.path),
  );

  const nextTheme = theme === "dark" ? "light" : "dark";

  return (
    <header className="topbar">
      <h1 className="topbar-title">{current?.label ?? "GameScape"}</h1>
      <div className="topbar-actions">
        <button
          type="button"
          className="icon-button"
          onClick={() => setTheme(nextTheme)}
          title={`Switch to ${nextTheme} theme`}
          aria-label={`Switch to ${nextTheme} theme`}
        >
          {theme === "dark" ? <Sun size={18} /> : <Moon size={18} />}
        </button>
      </div>
    </header>
  );
}
