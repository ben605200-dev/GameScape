import {
  Compass,
  Download,
  Home,
  Image,
  LayoutGrid,
  LayoutTemplate,
  Library,
  Settings,
  Star,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

export interface NavItem {
  path: string;
  label: string;
  icon: LucideIcon;
}

export const NAV_ITEMS: NavItem[] = [
  { path: "/", label: "Home", icon: Home },
  { path: "/wallpapers", label: "Wallpapers", icon: Image },
  { path: "/library", label: "Personal Library", icon: Library },
  { path: "/widgets", label: "Widgets", icon: LayoutGrid },
  { path: "/layouts", label: "Layout Editor", icon: LayoutTemplate },
  { path: "/discover", label: "Discover", icon: Compass },
  { path: "/downloads", label: "Downloads", icon: Download },
  { path: "/favourites", label: "Favourites", icon: Star },
  { path: "/settings", label: "Settings", icon: Settings },
];
