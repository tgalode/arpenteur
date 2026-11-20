// Placeholder remplacé par la tâche 13.
import { t } from "../i18n/fr";

export function Crawl({
  domain,
  id,
  onBack,
}: {
  domain: string;
  id: string;
  running: boolean;
  onBack: () => void;
}) {
  return (
    <main>
      <button type="button" onClick={onBack}>
        {t("back")}
      </button>
      <p>{domain}</p>
      <p>{id}</p>
    </main>
  );
}
