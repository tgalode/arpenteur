import { useState } from "react";
import { Crawl } from "./screens/Crawl";
import Home from "./screens/Home";
import Settings from "./screens/Settings";

type Nav = { screen: "home" } | { screen: "settings" } | { screen: "crawl"; domain: string; id: string; running: boolean };

function App() {
  const [nav, setNav] = useState<Nav>({ screen: "home" });
  const open = (domain: string, id: string, running: boolean) => setNav({ screen: "crawl", domain, id, running });
  if (nav.screen === "crawl") {
    return (
      <Crawl
        key={`${nav.domain}/${nav.id}`}
        domain={nav.domain}
        id={nav.id}
        running={nav.running}
        onBack={() => setNav({ screen: "home" })}
        onOpen={open}
      />
    );
  }
  if (nav.screen === "settings") {
    return <Settings onBack={() => setNav({ screen: "home" })} />;
  }
  return <Home onOpen={open} onSettings={() => setNav({ screen: "settings" })} />;
}

export default App;
