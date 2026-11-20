export const fr = {
  appName: "arpenteur",
  urlPlaceholder: "exemple.fr",
  urlLabel: "adresse du site",
  launch: "lancer",
  launching: "lancement…",
  sites: "# sites",
  noSites: "aucun crawl pour l’instant.",
  pages: "{count} URL",
  progress: "{done} URL traitées · {elapsed}",
  back: "retour",
  status_running: "en cours",
  status_completed: "terminé",
  status_stopped: "arrêté",
  status_throttled: "interrompu · le site limite les requêtes",
  status_failed: "échec",
  stop: "arrêter",
  restart: "relancer",
  open: "ouvrir",
  crawlsOne: "1 crawl",
  crawlsMany: "{count} crawls",
  loading: "chargement…",
  banner_stopped: "crawl arrêté avant la fin.",
  banner_throttled: "crawl interrompu : le site limite les requêtes. baissez le débit et relancez.",
  banner_failed: "le crawl a échoué : {message}",
  filterPlaceholder: "/ filtrer url, title, h1…",
  filterLabel: "filtre",
  statusFilterLabel: "classe de statut",
  statusAll: "statut : tous",
  statusError: "erreurs réseau",
  httpStatus_skipped: "ignorée",
  httpStatus_connection: "connexion impossible",
  httpStatus_timeout: "délai dépassé",
  httpStatus_network: "erreur réseau",
  col_url: "url",
  col_status: "statut",
  col_type: "type",
  col_title: "title",
  col_h1: "h1",
  col_indexable: "indexable",
  col_depth: "prof.",
  col_inlinks: "liens entr.",
  col_words: "mots",
  yes: "oui",
  no: "non",
  none: "–",
  noRows: "aucune URL ne correspond.",
  detail_label: "détail de {url}",
  version: "v{version}",
  detail_canonical: "canonical",
  detail_metaDescription: "meta description",
  detail_h1: "h1",
  detail_inlinks: "liens entrants",
  detail_outlinks: "liens sortants",
  detail_headers: "en-têtes",
  detail_jsonLd: "json-ld",
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
