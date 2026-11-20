import { useState } from "react";
import { Crawl } from "./screens/Crawl";
import Home from "./screens/Home";

type Nav = { screen: "home" } | { screen: "crawl"; domain: string; id: string; running: boolean };

function App() {
  const [nav, setNav] = useState<Nav>({ screen: "home" });
  if (nav.screen === "crawl") {
    return (
      <Crawl
        domain={nav.domain}
        id={nav.id}
        running={nav.running}
        onBack={() => setNav({ screen: "home" })}
      />
    );
  }
  return <Home onOpen={(domain, id, running) => setNav({ screen: "crawl", domain, id, running })} />;
}

export default App;
