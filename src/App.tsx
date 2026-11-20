import { useState } from "react";
import { Crawl } from "./screens/Crawl";
import Home from "./screens/Home";

type Nav = { screen: "home" } | { screen: "crawl"; domain: string; id: string; running: boolean };

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
  return <Home onOpen={open} />;
}

export default App;
