import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { WalletProvider } from "./contexts/WalletContext";
import { ChakraProvider } from "./providers/ChakraProvider";
import { DialogProvider } from "./contexts/DialogContext";
import { BlockingActionsProvider } from "./contexts/BlockingActionsContext";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ChakraProvider>
      <DialogProvider>
        <BlockingActionsProvider>
          <WalletProvider>
            <App />
          </WalletProvider>
        </BlockingActionsProvider>
      </DialogProvider>
    </ChakraProvider>
  </React.StrictMode>,
);
