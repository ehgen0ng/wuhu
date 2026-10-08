import React from "react";
import ReactDOM from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import App from "./app/App";
import { LanguageProvider } from "./app/LanguageProvider";
import { theme } from "./app/theme";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <MantineProvider theme={theme} defaultColorScheme="dark">
      <LanguageProvider>
        <App />
      </LanguageProvider>
    </MantineProvider>
  </React.StrictMode>,
);
