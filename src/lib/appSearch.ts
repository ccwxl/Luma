import type { AppInfo } from "./apps";

export type AppSortMode =
  | "name-asc"
  | "name-desc"
  | "installed-desc"
  | "installed-asc";

export const appSortOptions: ReadonlyArray<{
  value: AppSortMode;
  label: string;
}> = [
  { value: "installed-desc", label: "最新安装" },
  { value: "installed-asc", label: "最早安装" },
  { value: "name-asc", label: "首字母 A–Z" },
  { value: "name-desc", label: "首字母 Z–A" },
];

const nameCollator = new Intl.Collator(undefined, {
  numeric: true,
  sensitivity: "base",
});

function searchWords(value: string) {
  return value
    .replace(
      /(\p{Uppercase_Letter})(\p{Uppercase_Letter}\p{Lowercase_Letter})/gu,
      "$1 $2",
    )
    .replace(
      /([\p{Lowercase_Letter}\p{Number}])(\p{Uppercase_Letter})/gu,
      "$1 $2",
    )
    .normalize("NFKD")
    .toLocaleLowerCase()
    .replace(/\p{Mark}/gu, "")
    .split(/[^\p{Letter}\p{Number}]+/u)
    .filter(Boolean);
}

function compactSearchText(value: string) {
  return searchWords(value).join("");
}

function orderedCharacterScore(candidate: string, needle: string) {
  let cursor = 0;
  let firstMatch = -1;
  let previousMatch = -1;
  let gaps = 0;
  let consecutive = 0;

  for (const character of needle) {
    const match = candidate.indexOf(character, cursor);
    if (match < 0) return null;
    if (firstMatch < 0) firstMatch = match;
    if (previousMatch >= 0) {
      const gap = match - previousMatch - 1;
      gaps += gap;
      if (gap === 0) consecutive += 1;
    }
    previousMatch = match;
    cursor = match + 1;
  }

  return (
    consecutive * 12 -
    firstMatch * 8 -
    gaps * 4 -
    (candidate.length - needle.length)
  );
}

/**
 * Returns a relevance score for ordered-character fuzzy matching.
 * For example, `vc` matches both `VSCode` and `Visual Studio Code`.
 */
export function fuzzyAppNameScore(name: string, query: string) {
  const candidate = compactSearchText(name);
  const needle = compactSearchText(query);
  if (!needle) return 0;

  if (needle.length > 1) {
    const initials = searchWords(name)
      .map((word) => word[0])
      .join("");
    const acronymScore = orderedCharacterScore(initials, needle);
    if (acronymScore !== null) return 20_000 + acronymScore;
  }

  const exactIndex = candidate.indexOf(needle);
  if (exactIndex >= 0)
    return 10_000 - exactIndex * 20 - (candidate.length - needle.length);

  const fuzzyScore = orderedCharacterScore(candidate, needle);
  return fuzzyScore === null ? null : 1_000 + fuzzyScore;
}

function compareByName(a: AppInfo, b: AppInfo) {
  return nameCollator.compare(a.name, b.name) || a.path.localeCompare(b.path);
}

function compareByInstallTime(a: AppInfo, b: AppInfo, newestFirst: boolean) {
  const aTime = a.installed_at;
  const bTime = b.installed_at;
  const aKnown = typeof aTime === "number";
  const bKnown = typeof bTime === "number";
  if (aKnown !== bKnown) return aKnown ? -1 : 1;
  if (aKnown && bKnown && aTime !== bTime)
    return newestFirst ? bTime - aTime : aTime - bTime;
  return compareByName(a, b);
}

export function compareApps(a: AppInfo, b: AppInfo, mode: AppSortMode) {
  switch (mode) {
    case "name-desc":
      return -compareByName(a, b);
    case "installed-desc":
      return compareByInstallTime(a, b, true);
    case "installed-asc":
      return compareByInstallTime(a, b, false);
    default:
      return compareByName(a, b);
  }
}

export function filterAndSortApps(
  apps: AppInfo[],
  query: string,
  mode: AppSortMode,
) {
  const sorted = [...apps].sort((a, b) => compareApps(a, b, mode));
  if (!query.trim()) return sorted;

  return sorted
    .map((app, index) => ({
      app,
      index,
      score: fuzzyAppNameScore(app.name, query),
    }))
    .filter(
      (result): result is typeof result & { score: number } =>
        result.score !== null,
    )
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map(({ app }) => app);
}
