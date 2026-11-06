import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { Shell } from "../components/layout/Shell";
import { DiscoverPage } from "../pages/DiscoverPage";
import { DownloadsPage } from "../pages/DownloadsPage";
import { FavouritesPage } from "../pages/FavouritesPage";
import { HomePage } from "../pages/HomePage";
import { LayoutsPage } from "../pages/LayoutsPage";
import { LibraryItemPage } from "../pages/LibraryItemPage";
import { LibraryPage } from "../pages/LibraryPage";
import { SettingsPage } from "../pages/SettingsPage";
import { WallpaperDetailPage } from "../pages/WallpaperDetailPage";
import { WallpapersPage } from "../pages/WallpapersPage";
import { WidgetsPage } from "../pages/WidgetsPage";
import { FavouritesProvider } from "./FavouritesProvider";
import { ThemeProvider } from "./ThemeProvider";

export function App() {
  return (
    <ThemeProvider>
      <FavouritesProvider>
        <HashRouter>
        <Routes>
          <Route element={<Shell />}>
            <Route path="/" element={<HomePage />} />
            <Route path="/wallpapers" element={<WallpapersPage />} />
            <Route path="/wallpapers/:id" element={<WallpaperDetailPage />} />
            <Route path="/library" element={<LibraryPage />} />
            <Route path="/library/:id" element={<LibraryItemPage />} />
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
      </FavouritesProvider>
    </ThemeProvider>
  );
}
