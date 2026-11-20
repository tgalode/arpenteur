export const fr = {
  appName: "Arpenteur",
  urlPlaceholder: "https://exemple.fr",
  urlLabel: "Adresse du site",
  launch: "Lancer",
  launching: "Lancement…",
  sites: "Sites analysés",
  noSites: "Aucun crawl pour l’instant.",
  pages: "{count} pages",
  progress: "{done} URL traitées · {elapsed}",
  back: "Retour",
  status_running: "En cours",
  status_completed: "Terminé",
  status_stopped: "Arrêté",
  status_throttled: "Interrompu : le site limite les requêtes",
  status_failed: "Échec",
  stop: "Arrêter",
  loading: "Chargement…",
  banner_stopped: "Crawl arrêté avant la fin.",
  banner_throttled: "Crawl interrompu : le site limite les requêtes. Baissez le débit et relancez.",
  banner_failed: "Le crawl a échoué : {message}",
  filterPlaceholder: "Filtrer par URL, title ou H1",
  filterLabel: "Filtre",
  statusFilterLabel: "Classe de statut",
  statusAll: "Tous les statuts",
  statusError: "Erreurs réseau",
  httpStatus_skipped: "ignorée",
  httpStatus_connection: "connexion impossible",
  httpStatus_timeout: "délai dépassé",
  httpStatus_network: "erreur réseau",
  col_url: "URL",
  col_status: "Statut",
  col_type: "Type",
  col_title: "Title",
  col_h1: "H1",
  col_indexable: "Indexable",
  col_depth: "Profondeur",
  col_inlinks: "Liens entrants",
  col_words: "Mots",
  yes: "oui",
  no: "non",
  none: "–",
  noRows: "Aucune URL ne correspond.",
  detail_canonical: "Canonical",
  detail_metaDescription: "Meta description",
  detail_h1: "H1",
  detail_inlinks: "Liens entrants",
  detail_outlinks: "Liens sortants",
  detail_headers: "En-têtes de réponse",
  detail_jsonLd: "Types JSON-LD",
} as const;

export function t(key: keyof typeof fr, vars?: Record<string, string | number>): string {
  const text: string = fr[key];
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (m, name: string) => (name in vars ? String(vars[name]) : m));
}

export function formatElapsed(ms: number): string {
  const total = Math.floor(ms / 1000);
  if (total < 60) return `${total} s`;
  return `${Math.floor(total / 60)} min ${total % 60} s`;
}
