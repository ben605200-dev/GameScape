import { useEffect } from "react";
import { Outlet, useNavigate } from "react-router-dom";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { importFiles, IN_TAURI } from "../../lib/ipc";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";

export function Shell() {
  const navigate = useNavigate();

  // Global drag-and-drop: dropped files import no matter which page is open.
  useEffect(() => {
    if (!IN_TAURI) return;
    let unlisten: (() => void) | undefined;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          const paths = event.payload.paths ?? [];
          if (paths.length > 0) {
            window.dispatchEvent(
              new CustomEvent("gamescape:dropped", { detail: paths }),
            );
          }
        }
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {
        /* drag-drop unavailable in this environment — ignore */
      });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    const onDropped = async (event: Event) => {
      const paths = (event as CustomEvent<string[]>).detail ?? [];
      if (paths.length === 0) return;
      navigate("/library");
      await importFiles(paths);
      window.dispatchEvent(new CustomEvent("gamescape:library-refresh"));
    };
    window.addEventListener("gamescape:dropped", onDropped as EventListener);
    return () =>
      window.removeEventListener("gamescape:dropped", onDropped as EventListener);
  }, [navigate]);

  return (
    <div className="app-shell">
      <Sidebar />
      <div className="app-main">
        <TopBar />
        <main className="app-content">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
