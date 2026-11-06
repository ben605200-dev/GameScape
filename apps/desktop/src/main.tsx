import React from "react";
import ReactDOM from "react-dom/client";
import "@gamescape/ui/tokens.css";
import "./styles/app.css";
import { App } from "./app/App";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
