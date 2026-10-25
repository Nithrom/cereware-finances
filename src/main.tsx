import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/plus-jakarta-sans";
import "./styles/tokens.css";
import "./styles/base.css";
import App from "./App";
import { applyScale } from "./lib/uiscale";

applyScale();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
