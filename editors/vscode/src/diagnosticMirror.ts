export interface DiagnosticMirrorUri {
  scheme: string;
  path: string;
  query?: string;
}

export interface DiagnosticMirrorPosition {
  line: number;
  character: number;
}

export interface DiagnosticMirrorRange {
  start: DiagnosticMirrorPosition;
  end: DiagnosticMirrorPosition;
}

export interface DiagnosticMirrorLocation {
  uri: string;
  range: DiagnosticMirrorRange;
}

export interface DiagnosticMirrorDiagnostic {
  range: DiagnosticMirrorRange;
  message: string;
  severity?: unknown;
  code?: unknown;
  source?: string;
  tags?: readonly unknown[];
}

export interface EmbeddedDiagnosticSourceInfo {
  source: string;
  startLine: number;
}

export interface ShadowDocumentIdentity {
  content: string;
  uri: DiagnosticMirrorUri;
}

export type MirroredDiagnosticUriKind = 'generated' | 'embedded';

export interface MirroredDiagnosticPlan<
  TDiagnostic extends DiagnosticMirrorDiagnostic = DiagnosticMirrorDiagnostic,
> {
  location: DiagnosticMirrorLocation;
  diagnostic: TDiagnostic;
  fallbackSource: MirroredDiagnosticUriKind;
}

export interface MirroredDiagnosticCollectionEntry<TDiagnostic> {
  uri: string;
  diagnostics: TDiagnostic[];
}

export interface MirroredDiagnosticEvent<
  TDiagnostic extends DiagnosticMirrorDiagnostic = DiagnosticMirrorDiagnostic,
> {
  cacheKey: string;
  uri: DiagnosticMirrorUri;
  diagnostics: readonly TDiagnostic[];
}

export interface MirroredDiagnosticState<TMirroredDiagnostic> {
  cache: Map<string, readonly MirroredDiagnosticCollectionEntry<TMirroredDiagnostic>[]>;
}

export interface CollectMirroredDiagnosticCollectionOptions<
  TDiagnostic extends DiagnosticMirrorDiagnostic,
  TMirroredDiagnostic,
> extends CollectMirroredDiagnosticsOptions {
  createDiagnostic: (args: {
    location: DiagnosticMirrorLocation;
    diagnostic: TDiagnostic;
    fallbackSource: MirroredDiagnosticUriKind;
  }) => TMirroredDiagnostic;
}

export interface CollectMirroredDiagnosticsOptions {
  remapGeneratedRange: (
    uri: DiagnosticMirrorUri,
    range: DiagnosticMirrorRange
  ) => Promise<DiagnosticMirrorLocation | undefined>;
}

export function classifyMirroredDiagnosticUri(
  uri: DiagnosticMirrorUri
): MirroredDiagnosticUriKind | undefined {
  if (uri.scheme === 'mds-embedded') {
    return 'embedded';
  }
  if (uri.scheme === 'file' && !uri.path.endsWith('.md')) {
    return 'generated';
  }
  return undefined;
}

export function parseDiagnosticMirrorUri(
  uriText: string
): DiagnosticMirrorUri | undefined {
  try {
    const parsed = new URL(uriText);
    return {
      scheme: parsed.protocol.replace(/:$/, ''),
      path: decodeURIComponent(parsed.pathname),
      query: parsed.search.startsWith('?') ? parsed.search.slice(1) : '',
    };
  } catch {
    return undefined;
  }
}

export function isMarkdownDiagnosticUri(uri: DiagnosticMirrorUri): boolean {
  return uri.scheme === 'file' && uri.path.endsWith('.md');
}

export function shadowDocumentIdentityEqual(
  left: ShadowDocumentIdentity,
  right: ShadowDocumentIdentity
): boolean {
  return left.content === right.content
    && left.uri.scheme === right.uri.scheme
    && left.uri.path === right.uri.path
    && (left.uri.query || '') === (right.uri.query || '');
}

export function parseEmbeddedDiagnosticSourceInfo(
  query: string
): EmbeddedDiagnosticSourceInfo | undefined {
  const params = new URLSearchParams(query);
  const source = params.get('source');
  const startLineValue = params.get('startLine');
  if (!source || !startLineValue) {
    return undefined;
  }

  const startLine = Number.parseInt(startLineValue, 10);
  if (!Number.isInteger(startLine) || startLine < 0) {
    return undefined;
  }

  return {
    source,
    startLine,
  };
}

export function remapEmbeddedDiagnosticRange(
  startLine: number,
  range: DiagnosticMirrorRange
): DiagnosticMirrorRange {
  return {
    start: {
      line: range.start.line + startLine,
      character: range.start.character,
    },
    end: {
      line: range.end.line + startLine,
      character: range.end.character,
    },
  };
}

export async function collectMirroredDiagnostics<
  TDiagnostic extends DiagnosticMirrorDiagnostic,
>(
  uri: DiagnosticMirrorUri,
  diagnostics: readonly TDiagnostic[],
  options: CollectMirroredDiagnosticsOptions
): Promise<MirroredDiagnosticPlan<TDiagnostic>[]> {
  const uriKind = classifyMirroredDiagnosticUri(uri);
  if (uriKind === 'generated') {
    return collectGeneratedMirroredDiagnostics(uri, diagnostics, options);
  }
  if (uriKind === 'embedded') {
    return collectEmbeddedMirroredDiagnostics(uri, diagnostics);
  }
  return [];
}

export async function collectMirroredDiagnosticCollection<
  TDiagnostic extends DiagnosticMirrorDiagnostic,
  TMirroredDiagnostic,
>(
  uri: DiagnosticMirrorUri,
  diagnostics: readonly TDiagnostic[],
  options: CollectMirroredDiagnosticCollectionOptions<
    TDiagnostic,
    TMirroredDiagnostic
  >
): Promise<MirroredDiagnosticCollectionEntry<TMirroredDiagnostic>[]> {
  const collection = new Map<
    string,
    MirroredDiagnosticCollectionEntry<TMirroredDiagnostic>
  >();
  const mirroredDiagnostics = await collectMirroredDiagnostics(
    uri,
    diagnostics,
    options
  );

  for (const mirroredDiagnostic of mirroredDiagnostics) {
    const key = mirroredDiagnostic.location.uri;
    const existing = collection.get(key);
    const mappedDiagnostic = options.createDiagnostic({
      location: mirroredDiagnostic.location,
      diagnostic: mirroredDiagnostic.diagnostic,
      fallbackSource: mirroredDiagnostic.fallbackSource,
    });

    if (existing) {
      existing.diagnostics.push(mappedDiagnostic);
      continue;
    }

    collection.set(key, {
      uri: key,
      diagnostics: [mappedDiagnostic],
    });
  }

  return [...collection.values()];
}

export function mergeMirroredDiagnosticCollections<TDiagnostic>(
  collections: Iterable<readonly MirroredDiagnosticCollectionEntry<TDiagnostic>[]>
): MirroredDiagnosticCollectionEntry<TDiagnostic>[] {
  const aggregate = new Map<string, MirroredDiagnosticCollectionEntry<TDiagnostic>>();

  for (const collection of collections) {
    for (const entry of collection) {
      const existing = aggregate.get(entry.uri);
      if (existing) {
        existing.diagnostics.push(...entry.diagnostics);
        continue;
      }

      aggregate.set(entry.uri, {
        uri: entry.uri,
        diagnostics: [...entry.diagnostics],
      });
    }
  }

  return [...aggregate.values()];
}

export function mirroredDiagnosticCollectionsEqual<TDiagnostic>(
  left: readonly MirroredDiagnosticCollectionEntry<TDiagnostic>[],
  right: readonly MirroredDiagnosticCollectionEntry<TDiagnostic>[],
  isDiagnosticEqual: (left: TDiagnostic, right: TDiagnostic) => boolean = Object.is
): boolean {
  if (left === right) {
    return true;
  }

  if (left.length !== right.length) {
    return false;
  }

  for (let entryIndex = 0; entryIndex < left.length; entryIndex += 1) {
    const leftEntry = left[entryIndex];
    const rightEntry = right[entryIndex];

    if (leftEntry.uri !== rightEntry.uri) {
      return false;
    }

    if (leftEntry.diagnostics.length !== rightEntry.diagnostics.length) {
      return false;
    }

    for (
      let diagnosticIndex = 0;
      diagnosticIndex < leftEntry.diagnostics.length;
      diagnosticIndex += 1
    ) {
      if (!isDiagnosticEqual(
        leftEntry.diagnostics[diagnosticIndex],
        rightEntry.diagnostics[diagnosticIndex]
      )) {
        return false;
      }
    }
  }

  return true;
}

export function createMirroredDiagnosticState<TMirroredDiagnostic>(): MirroredDiagnosticState<TMirroredDiagnostic> {
  return {
    cache: new Map(),
  };
}

export function purgeMirroredDiagnosticCacheKeys<TMirroredDiagnostic>(
  state: MirroredDiagnosticState<TMirroredDiagnostic>,
  cacheKeys: readonly string[]
): MirroredDiagnosticCollectionEntry<TMirroredDiagnostic>[] {
  for (const cacheKey of new Set(cacheKeys)) {
    state.cache.delete(cacheKey);
  }

  return mergeMirroredDiagnosticCollections(state.cache.values());
}

export async function applyMirroredDiagnosticEvents<
  TDiagnostic extends DiagnosticMirrorDiagnostic,
  TMirroredDiagnostic,
>(
  state: MirroredDiagnosticState<TMirroredDiagnostic>,
  events: readonly MirroredDiagnosticEvent<TDiagnostic>[],
  options: CollectMirroredDiagnosticCollectionOptions<
    TDiagnostic,
    TMirroredDiagnostic
  >
): Promise<MirroredDiagnosticCollectionEntry<TMirroredDiagnostic>[]> {
  const mirroredEvents = [...new Map(
    events
      .filter((event) => !!classifyMirroredDiagnosticUri(event.uri))
      .map((event) => [event.cacheKey, event])
  ).values()];

  if (mirroredEvents.length === 0) {
    return mergeMirroredDiagnosticCollections(state.cache.values());
  }

  await Promise.all(
    mirroredEvents.map(async (event) => {
      const remapped = await collectMirroredDiagnosticCollection(
        event.uri,
        event.diagnostics,
        options
      );

      if (remapped.length === 0) {
        state.cache.delete(event.cacheKey);
        return;
      }

      state.cache.set(event.cacheKey, remapped);
    })
  );

  return mergeMirroredDiagnosticCollections(state.cache.values());
}

async function collectGeneratedMirroredDiagnostics<
  TDiagnostic extends DiagnosticMirrorDiagnostic,
>(
  uri: DiagnosticMirrorUri,
  diagnostics: readonly TDiagnostic[],
  options: CollectMirroredDiagnosticsOptions
): Promise<MirroredDiagnosticPlan<TDiagnostic>[]> {
  const remapped: Array<MirroredDiagnosticPlan<TDiagnostic> | undefined> = await Promise.all(
    diagnostics.map(async (diagnostic) => {
      const location = await options.remapGeneratedRange(uri, diagnostic.range);
      if (!location) {
        return undefined;
      }

      const parsedLocationUri = parseDiagnosticMirrorUri(location.uri);
      if (!parsedLocationUri || !isMarkdownDiagnosticUri(parsedLocationUri)) {
        return undefined;
      }

      return {
        location,
        diagnostic,
        fallbackSource: 'generated' as const,
      };
    })
  );

  return remapped.flatMap((plan) => (plan ? [plan] : []));
}

function collectEmbeddedMirroredDiagnostics<
  TDiagnostic extends DiagnosticMirrorDiagnostic,
>(
  uri: DiagnosticMirrorUri,
  diagnostics: readonly TDiagnostic[]
): MirroredDiagnosticPlan<TDiagnostic>[] {
  const sourceInfo = parseEmbeddedDiagnosticSourceInfo(uri.query || '');
  if (!sourceInfo) {
    return [];
  }

  const sourceUri = parseDiagnosticMirrorUri(sourceInfo.source);
  if (!sourceUri || !isMarkdownDiagnosticUri(sourceUri)) {
    return [];
  }

  return diagnostics.map((diagnostic) => ({
    location: {
      uri: sourceInfo.source,
      range: remapEmbeddedDiagnosticRange(sourceInfo.startLine, diagnostic.range),
    },
    diagnostic,
    fallbackSource: 'embedded',
  }));
}