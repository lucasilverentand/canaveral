import React from "react";
import { createRoot } from "react-dom/client";

const App = () => (
  <main>
    <h1>{{project_name}} / {{block_name}} dashboard</h1>
    <p>API: {{api_block_name}}</p>
  </main>
);

createRoot(document.getElementById("root")!).render(<App />);
