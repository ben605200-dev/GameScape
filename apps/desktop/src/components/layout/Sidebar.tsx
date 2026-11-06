import { NavLink } from "react-router-dom";
import { NAV_ITEMS } from "../../app/nav";

export function Sidebar() {
  return (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <span className="brand-mark" aria-hidden="true">
          G
        </span>
        <div className="brand-text">
          <span className="brand-name">GameScape</span>
          <span className="brand-tag">pre-alpha</span>
        </div>
      </div>

      <nav className="nav" aria-label="Main navigation">
        {NAV_ITEMS.map(({ path, label, icon: Icon }) => (
          <NavLink
            key={path}
            to={path}
            end={path === "/"}
            className={({ isActive }) =>
              `nav-item${isActive ? " nav-item-active" : ""}`
            }
          >
            <Icon size={17} strokeWidth={1.9} aria-hidden="true" />
            <span>{label}</span>
          </NavLink>
        ))}
      </nav>

      <div className="sidebar-foot">v0.0.0 · foundation build</div>
    </aside>
  );
}
