import { useState } from "react";
import type { PluginManifest } from "@rotameter/plugin-sdk";
import { InstrumentFrame, Tabs } from "@rotameter/ui";
import { KeysPanel } from "./KeysPanel";
import { SitesPanel } from "./SitesPanel";

function AdminPluginView() {
  const [tab, setTab] = useState("sites");

  return (
    <InstrumentFrame
      title="Control Plane"
      subtitle="Sites, collector snippets, and scoped API keys"
      reticles
    >
      <Tabs
        value={tab}
        onValueChange={setTab}
        items={[
          { value: "sites", label: "Sites", content: <SitesPanel /> },
          { value: "keys", label: "API Keys", content: <KeysPanel /> },
        ]}
      />
    </InstrumentFrame>
  );
}

const manifest: PluginManifest = {
  id: "admin",
  name: "Admin",
  version: "0.3.0",
  required_scopes: ["admin:sites", "admin:keys"],
  nav_items: [{ id: "admin", label: "Admin", path: "/admin", order: 80 }],
  routes: [{ id: "admin", path: "/admin", component: AdminPluginView }],
};

export default manifest;
