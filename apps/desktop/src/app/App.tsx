import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { Shell } from "../components/layout/Shell";
import { DiscoverPage } from "../pages/DiscoverPage";
import { DownloadsPage } from "../pages/DownloadsPage";
import { FavouritesPage } from "../pages/FavouritesPage";
import { HomePage } from "../pages/HomePage";
import { LayoutsPage } from "../pages/LayoutsPage";
import { LibraryPage } from "../pages/LibraryPage";
import { SettingsPage } from "../pages/SettingsPage";
import { WallpapersPage } from "../pages/WallpapersPage";
import { WidgetsPage } from "../pages/WidgetsPage";
import { ThemeProvider } from "./ThemeProvider";

export function App() {
  return (
    <ThemeProvider>
      <HashRouter>
        <Routes>
          <Route element={<Shell />}>
            <Route path="/" element={<HomePage />} />
            <Route path="/wallpapers" element={<WallpapersPage />} />
            <Route path="/library" element={<LibraryPage />} />
            <Route path="/widgets" element={<WidgetsPage />} />
            <Route path="/layouts" element={<LayoutsPage />} />
            <Route path="/discover" element={<DiscoverPage />} />
            <Route path="/downloads" element={<DownloadsPage />} />
            <Route path="/favourites" element={<FavouritesPage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Route>
        </Routes>
      </HashRouter>
    </ThemeProvider>
  );
}
