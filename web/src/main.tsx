import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/geist/400.css";
import "@fontsource/geist/500.css";
import "@fontsource/instrument-serif/latin-400.css";
import "@fontsource/instrument-serif/latin-400-italic.css";
import "@fontsource/jetbrains-mono/latin-400.css";
import "./components/silicon-ui/foundation.css";
import "./silicon-ui-compat.css";
import "./styles.css";
import App from "./App";
import { applyTheme } from "./theme";
applyTheme();
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
