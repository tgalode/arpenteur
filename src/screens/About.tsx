import { t } from "../i18n/fr";
import { version } from "../../package.json";
import siteoneLicence from "../licenses/siteone-crawler.txt?raw";

export default function About({ onBack }: { onBack: () => void }) {
  return (
    <main className="settings">
      <header className="topbar">
        <button type="button" className="btn" onClick={onBack}>
          {t("back")}
        </button>
        <span className="logo">{t("appName")}</span>
        <span className="dim">{t("version", { version })}</span>
      </header>
      <div className="settings-body">
        <h2 className="home-h dim">{t("about_title")}</h2>
        <p className="settings-intro">{t("about_summary")}</p>
        <h2 className="home-h dim">{t("about_thirdParty")}</h2>
        <h3 className="strong">{t("about_engine")}</h3>
        <p className="dim settings-intro">{t("about_engineIntro")}</p>
        <pre className="licence">{siteoneLicence}</pre>
      </div>
    </main>
  );
}
