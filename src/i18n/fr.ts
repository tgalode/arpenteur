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
} as const;

export function t(key: keyof typeof fr, vars?: Record<string, string | number>): string {
  const text: string = fr[key];
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (m, name: string) => (name in vars ? String(vars[name]) : m));
}
