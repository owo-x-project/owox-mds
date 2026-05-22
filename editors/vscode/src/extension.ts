import * as vscode from 'vscode';
import {
  applyMirroredDiagnosticEvents,
  classifyMirroredDiagnosticUri,
  createMirroredDiagnosticState,
  mirroredDiagnosticCollectionsEqual,
  parseEmbeddedDiagnosticSourceInfo,
  purgeMirroredDiagnosticCacheKeys,
  shadowDocumentIdentityEqual,
  type DiagnosticMirrorDiagnostic,
  type MirroredDiagnosticCollectionEntry,
  type DiagnosticMirrorUri,
  type MirroredDiagnosticUriKind,
  type ShadowDocumentIdentity,
} from './diagnosticMirror';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

// ============================================================
// Language Registry
// ============================================================

interface LanguageInfo {
  /** mds file extension (e.g., '.ts.md') */
  ext: string;
  /** VS Code language ID (e.g., 'typescript') */
  languageId: string;
  /** Code block labels that identify this language */
  labels: string[];
  /** Markdown path suffixes used by *.suffix.md authoring files */
  markdownSuffixes: string[];
  /** Virtual file extension for embedded docs (e.g., '.ts') */
  virtualExt: string;
}

let LANGUAGE_REGISTRY: Record<string, LanguageInfo> = {};

/** Dynamically create a LanguageInfo for any unknown extension */
function createDynamicLanguageInfo(key: string): LanguageInfo {
  return {
    ext: `.${key}.md`,
    languageId: key,
    labels: [key],
    markdownSuffixes: [key],
    virtualExt: `.${key}`,
  };
}

let LANG_ALIASES: Record<string, string> = {};

function normalizeLangKey(key: string): string {
  const lower = key.toLowerCase();
  return LANG_ALIASES[lower] || lower;
}

function registerLanguageInfo(info: LanguageInfo): LanguageInfo {
  const labels = [...new Set(
    info.labels
      .map((label) => label.trim())
      .filter((label) => label.length > 0)
  )];
  const markdownSuffixes = [...new Set(
    info.markdownSuffixes
      .map((suffix) => suffix.trim().replace(/^\./, '').replace(/\.md$/i, ''))
      .filter((suffix) => suffix.length > 0)
  )];
  const fallbackKey = markdownSuffixes[0] || info.virtualExt.replace(/^\./, '') || info.languageId;
  const canonicalKey = normalizeLangKey(labels[0] || fallbackKey);
  const registered: LanguageInfo = {
    ...info,
    labels: labels.length > 0 ? labels : [canonicalKey],
    markdownSuffixes: markdownSuffixes.length > 0 ? markdownSuffixes : [fallbackKey],
  };

  LANGUAGE_REGISTRY[canonicalKey] = registered;
  for (const label of registered.labels) {
    LANG_ALIASES[label.toLowerCase()] = canonicalKey;
  }

  return registered;
}

function registerDiscoveredLanguages(
  languages: readonly LanguageInfo[]
): LanguageInfo[] {
  const registered = languages.map((language) => registerLanguageInfo(language));
  labelToLang = buildLabelMap();
  return registered;
}

/** Build a label → LanguageInfo lookup map */
function buildLabelMap(): Map<string, LanguageInfo> {
  const map = new Map<string, LanguageInfo>();
  for (const info of Object.values(LANGUAGE_REGISTRY)) {
    for (const label of info.labels) {
      map.set(label.toLowerCase(), info);
    }
  }
  return map;
}

let labelToLang = buildLabelMap();

function resetLanguageRegistry(): void {
  LANGUAGE_REGISTRY = {};
  LANG_ALIASES = {};
  labelToLang = buildLabelMap();
}

const CANONICAL_SOURCE_MD_ROOT = '.mds/source';
const CANONICAL_TEST_MD_ROOT = '.mds/test';
const OVERVIEW_MARKDOWN_NAME = 'overview.md';
const PACKAGE_MARKDOWN_NAME = 'package.md';

function resolveCanonicalAuthoringRoot(
  text: string,
  key: 'source_md' | 'test_md',
  canonicalRoot: string
): string {
  const configuredRoot = sectionStringField(text, 'roots', key);
  return configuredRoot === canonicalRoot ? configuredRoot : canonicalRoot;
}

interface AuthoringRoots {
  source: string[];
  test: string[];
}

interface ActiveDocumentContext {
  activeLanguage: string;
  docKind: string;
}

interface ActiveContextStatusBarController {
  refresh(editor?: vscode.TextEditor): void;
}

interface ExtensionRuntimeState {
  activeLanguages: LanguageInfo[];
  authoringRoots: AuthoringRoots;
}

// ============================================================
// Config-based Language Discovery
// ============================================================

/**
 * Discover active languages from mds.config.toml files in the workspace
 * and user settings. Core languages (ts, py, rs) are always included.
 */
async function discoverLanguages(
  config: vscode.WorkspaceConfiguration
): Promise<LanguageInfo[]> {
  resetLanguageRegistry();
  await discoverDescriptorLanguages();
  const langKeys = new Set<string>(Object.keys(LANGUAGE_REGISTRY));

  // From user settings (additionalLanguages: ['.go.md', '.java.md'])
  for (const ext of config.get<string[]>('additionalLanguages', [])) {
    const m = ext.match(/^\.([A-Za-z0-9._-]+)\.md$/);
    if (m) {
      langKeys.add(normalizeLangKey(m[1]));
    }
  }

  // From workspace mds.config.toml files
  try {
    const files = await vscode.workspace.findFiles(
      '**/mds.config.toml',
      '{**/node_modules/**,**/target/**}'
    );
    for (const uri of files) {
      try {
        const bytes = await vscode.workspace.fs.readFile(uri);
        const text = new TextDecoder().decode(bytes);
        // Match [quality.LANG] or [adapters.LANG] sections
        const re = /\[(?:quality|adapters)\.(\w+)\]/g;
        let match;
        while ((match = re.exec(text)) !== null) {
          langKeys.add(normalizeLangKey(match[1]));
        }
      } catch {
        // Skip unreadable files
      }
    }
  } catch {
    // Workspace search not available
  }

  // From actual .{ext}.md files in mds authoring roots
  try {
    for (const pattern of [
      '**/.mds/source/**/*.md',
      '**/.mds/test/**/*.md',
    ]) {
      const mdFiles = await vscode.workspace.findFiles(
        pattern,
        '{**/node_modules/**,**/target/**}'
      );
      for (const uri of mdFiles) {
        const fileName = uri.path.split('/').pop() || '';
        const m = fileName.match(/^[^.]+\.([A-Za-z0-9._-]+)\.md$/);
        if (m) {
          langKeys.add(normalizeLangKey(m[1]));
        }
      }
    }
  } catch {
    // Workspace search not available
  }

  return registerDiscoveredLanguages(
    [...langKeys].map((k) => LANGUAGE_REGISTRY[k] || createDynamicLanguageInfo(k))
  );
}

async function discoverDescriptorLanguages(): Promise<void> {
  try {
    const files = await vscode.workspace.findFiles(
      '**/.mds/descriptors/languages/**/*.toml',
      '{**/node_modules/**,**/target/**}'
    );
    for (const uri of files) {
      try {
        const text = new TextDecoder().decode(await vscode.workspace.fs.readFile(uri));
        const info = languageInfoFromDescriptor(text);
        if (info) {
          registerLanguageInfo(info);
        }
      } catch {
        // Skip unreadable or incomplete descriptors.
      }
    }
    labelToLang = buildLabelMap();
  } catch {
    // Workspace search not available.
  }
}

function languageInfoFromDescriptor(text: string): LanguageInfo | undefined {
  const id = stringField(text, 'id');
  const primaryExt = sectionStringField(text, 'language', 'primary_ext') || id;
  if (!id || !primaryExt) {
    return undefined;
  }
  const aliases = arrayField(text, 'aliases');
  const markdownSuffixes = arrayField(text, 'match_suffixes');
  const labels = [...new Set([id, ...aliases, primaryExt, ...markdownSuffixes])];
  const vscodeId = sectionStringField(text, 'language', 'vscode_id') || id;
  return {
    ext: `.${primaryExt}.md`,
    languageId: vscodeId,
    labels,
    markdownSuffixes: markdownSuffixes.length > 0 ? markdownSuffixes : [primaryExt],
    virtualExt: `.${primaryExt}`,
  };
}

function stringField(text: string, key: string): string | undefined {
  return text.match(new RegExp(`^${key}\\s*=\\s*"([^"]+)"`, 'm'))?.[1];
}

function sectionStringField(text: string, section: string, key: string): string | undefined {
  const match = text.match(new RegExp(`\\[${section}\\]([\\s\\S]*?)(?:\\n\\[|$)`));
  return match ? stringField(match[1], key) : undefined;
}

function arrayField(text: string, key: string): string[] {
  const values = text.match(new RegExp(`^${key}\\s*=\\s*\\[([^\\]]*)\\]`, 'm'))?.[1] || '';
  return [...values.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
}

async function discoverAuthoringRoots(): Promise<AuthoringRoots> {
  const path = require('path') as typeof import('path');
  const source = new Set<string>();
  const test = new Set<string>();

  try {
    const files = await vscode.workspace.findFiles(
      '**/mds.config.toml',
      '{**/node_modules/**,**/target/**}'
    );
    for (const uri of files) {
      try {
        const bytes = await vscode.workspace.fs.readFile(uri);
        const text = new TextDecoder().decode(bytes);
        const packageRoot = path.dirname(uri.fsPath);
        const sourceRoot = resolveCanonicalAuthoringRoot(
          text,
          'source_md',
          CANONICAL_SOURCE_MD_ROOT
        );
        const testRoot = resolveCanonicalAuthoringRoot(
          text,
          'test_md',
          CANONICAL_TEST_MD_ROOT
        );
        source.add(
          normalizeFsPath(
            path.resolve(
              packageRoot,
              sourceRoot
            )
          )
        );
        test.add(
          normalizeFsPath(
            path.resolve(
              packageRoot,
              testRoot
            )
          )
        );
      } catch {
        // Skip unreadable config files.
      }
    }
  } catch {
    // Workspace search not available.
  }

  return {
    source: [...source],
    test: [...test],
  };
}

function normalizeFsPath(fsPath: string): string {
  return fsPath.replace(/\\/g, '/');
}

function isInsideResolvedRoot(fsPath: string, roots: string[]): boolean {
  const normalizedPath = normalizeFsPath(fsPath);
  return roots.some((root) => {
    const normalizedRoot = normalizeFsPath(root).replace(/\/+$/, '');
    return normalizedPath === normalizedRoot || normalizedPath.startsWith(`${normalizedRoot}/`);
  });
}

function isInSourceRoot(fsPath: string, authoringRoots: AuthoringRoots): boolean {
  return isInsideResolvedRoot(fsPath, authoringRoots.source);
}

function isInTestRoot(fsPath: string, authoringRoots: AuthoringRoots): boolean {
  return isInsideResolvedRoot(fsPath, authoringRoots.test);
}

function languageDisplayLabel(language: LanguageInfo): string {
  return language.labels[0] || language.languageId || language.virtualExt.replace(/^\./, '');
}

function resolvePathFallbackLanguage(
  document: vscode.TextDocument,
  activeLanguages: readonly LanguageInfo[]
): string {
  const fileName = document.fileName.toLowerCase();
  const matches = activeLanguages.flatMap((language) =>
    language.markdownSuffixes
      .map((suffix) => suffix.toLowerCase())
      .filter((suffix) => fileName.endsWith(`.${suffix}.md`))
      .map((suffix) => ({ language, suffix }))
  );
  if (matches.length === 0) {
    return 'unknown';
  }

  const longestSuffixLength = matches
    .reduce((longest, match) => Math.max(longest, match.suffix.length), 0);
  const resolvedLanguages = [...new Set(
    matches
      .filter((match) => match.suffix.length === longestSuffixLength)
      .map((match) => languageDisplayLabel(match.language))
  )];

  return resolvedLanguages.length === 1 ? resolvedLanguages[0] : 'unknown';
}

function isManagedMarkdownAuthoringPath(
  fsPath: string,
  authoringRoots: AuthoringRoots
): boolean {
  const normalizedPath = normalizeFsPath(fsPath);
  return normalizedPath.toLowerCase().endsWith('.md')
    && (isInSourceRoot(normalizedPath, authoringRoots)
      || isInTestRoot(normalizedPath, authoringRoots));
}

function isManagedMarkdownAuthoringDocument(
  document: vscode.TextDocument,
  authoringRoots: AuthoringRoots
): boolean {
  return document.uri.scheme === 'file'
    && isManagedMarkdownAuthoringPath(document.fileName, authoringRoots);
}

function resolveDocKind(uri: vscode.Uri, authoringRoots: AuthoringRoots): string {
  const fsPath = uri.fsPath;
  const name = uri.path.split('/').pop()?.toLowerCase() || '';

  if (name === PACKAGE_MARKDOWN_NAME) {
    return 'package';
  }
  if (isInTestRoot(fsPath, authoringRoots)) {
    return name === OVERVIEW_MARKDOWN_NAME ? 'test-overview' : 'test';
  }
  if (isInSourceRoot(fsPath, authoringRoots)) {
    return name === OVERVIEW_MARKDOWN_NAME ? 'source-overview' : 'source';
  }
  if (name === OVERVIEW_MARKDOWN_NAME) {
    return 'overview';
  }
  return 'unknown';
}

function resolveActiveDocumentContext(
  editor: vscode.TextEditor | undefined,
  activeLanguages: readonly LanguageInfo[],
  authoringRoots: AuthoringRoots
): ActiveDocumentContext | undefined {
  if (!editor) {
    return undefined;
  }

  const { document } = editor;
  if (!isManagedMarkdownAuthoringDocument(document, authoringRoots)) {
    return undefined;
  }

  const block = findBlockAtPosition(document, editor.selection.active.line);
  const activeLanguage = block
    ? block.languageLabel
    : resolvePathFallbackLanguage(document, activeLanguages);

  return {
    activeLanguage: activeLanguage || 'unknown',
    docKind: resolveDocKind(document.uri, authoringRoots),
  };
}

function registerActiveContextStatusBar(
  context: vscode.ExtensionContext,
  runtimeState: ExtensionRuntimeState
): ActiveContextStatusBarController {
  const statusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Right,
    100
  );
  statusBarItem.name = 'mds Active Context';

  const refresh = (editor: vscode.TextEditor | undefined = vscode.window.activeTextEditor) => {
    const activeContext = resolveActiveDocumentContext(
      editor,
      runtimeState.activeLanguages,
      runtimeState.authoringRoots
    );

    if (!activeContext) {
      statusBarItem.hide();
      return;
    }

    statusBarItem.text = `mds ${activeContext.activeLanguage} | ${activeContext.docKind}`;
    statusBarItem.tooltip = `mds active language: ${activeContext.activeLanguage}\ndoc kind: ${activeContext.docKind}`;
    statusBarItem.show();
  };

  context.subscriptions.push(
    statusBarItem,
    vscode.window.onDidChangeActiveTextEditor((editor) => refresh(editor)),
    vscode.window.onDidChangeTextEditorSelection((event) => refresh(event.textEditor)),
    vscode.workspace.onDidOpenTextDocument(() => refresh()),
    vscode.workspace.onDidCloseTextDocument(() => refresh()),
    vscode.workspace.onDidChangeTextDocument(() => refresh())
  );

  refresh();

  return { refresh };
}

// ============================================================
// Code Block Parsing
// ============================================================

interface CodeBlock {
  /** VS Code language ID (e.g., 'typescript') */
  languageId: string;
  /** Language label shown in mds context (e.g., 'ts', 'typescript') */
  languageLabel: string;
  /** Virtual file extension (e.g., '.ts') */
  virtualExt: string;
  /** First content line (line after opening ```) */
  startLine: number;
  /** Last content line (line before closing ```) */
  endLine: number;
  /** Index among all code blocks in the document */
  index: number;
}

interface EmbeddedDocumentProvider extends vscode.TextDocumentContentProvider {
  refresh(uri: vscode.Uri): void;
}

function parseCodeBlocks(document: vscode.TextDocument): CodeBlock[] {
  const blocks: CodeBlock[] = [];
  let inBlock = false;
  let fenceLen = 0;
  let langId = '';
  let langLabel = '';
  let vExt = '';
  let start = 0;

  for (let i = 0; i < document.lineCount; i++) {
    const text = document.lineAt(i).text;
    if (!inBlock) {
      const opened = parseFenceLine(text);
      if (opened && opened.label) {
        const label = opened.label.toLowerCase();
        const normalizedLabel = normalizeLangKey(label);
        const info = labelToLang.get(label) || labelToLang.get(normalizedLabel);
        inBlock = true;
        fenceLen = opened.markerLen;
        langId = info?.languageId || normalizedLabel;
        langLabel = info ? info.labels[0] || 'unknown' : 'unknown';
        vExt = info?.virtualExt || `.${normalizedLabel}`;
        start = i + 1;
      }
    } else if (isClosingFence(text, fenceLen)) {
      if (i > start) {
        blocks.push({
          languageId: langId,
          languageLabel: langLabel,
          virtualExt: vExt,
          startLine: start,
          endLine: i - 1,
          index: blocks.length,
        });
      }
      inBlock = false;
      fenceLen = 0;
    }
  }
  return blocks;
}

function parseFenceLine(text: string): { markerLen: number; label: string } | undefined {
  const trimmed = text.trimStart();
  const markerLen = trimmed.match(/^`*/)?.[0].length || 0;
  if (markerLen < 3) {
    return undefined;
  }
  const rest = trimmed.slice(markerLen).trim();
  const label = rest.split(/\s+/)[0] || '';
  return { markerLen, label };
}

function isClosingFence(text: string, openLen: number): boolean {
  const parsed = parseFenceLine(text);
  return !!parsed && parsed.markerLen >= openLen && parsed.label === '';
}

/** Cached code blocks per document URI */
const blockCache = new Map<string, CodeBlock[]>();

function getCodeBlocks(document: vscode.TextDocument): CodeBlock[] {
  const key = document.uri.toString();
  let blocks = blockCache.get(key);
  if (!blocks) {
    blocks = parseCodeBlocks(document);
    blockCache.set(key, blocks);
  }
  return blocks;
}

function findBlockAtPosition(
  document: vscode.TextDocument,
  line: number
): CodeBlock | undefined {
  return getCodeBlocks(document).find(
    (b) => line >= b.startLine && line <= b.endLine
  );
}

function rangeTerminalLine(range: vscode.Range): number {
  if (range.end.character === 0 && range.end.line > range.start.line) {
    return range.end.line - 1;
  }
  return range.end.line;
}

function findBlockForRange(
  document: vscode.TextDocument,
  range: vscode.Range
): CodeBlock | undefined {
  const startBlock = findBlockAtPosition(document, range.start.line);
  if (!startBlock) {
    return undefined;
  }

  const endBlock = findBlockAtPosition(document, rangeTerminalLine(range));
  return endBlock && endBlock.index === startBlock.index ? startBlock : undefined;
}

function findManagedBlockAtPosition(
  document: vscode.TextDocument,
  line: number,
  authoringRoots: AuthoringRoots
): CodeBlock | undefined {
  if (!isManagedMarkdownAuthoringDocument(document, authoringRoots)) {
    return undefined;
  }
  return findBlockAtPosition(document, line);
}

function findManagedBlockForRange(
  document: vscode.TextDocument,
  range: vscode.Range,
  authoringRoots: AuthoringRoots
): CodeBlock | undefined {
  if (!isManagedMarkdownAuthoringDocument(document, authoringRoots)) {
    return undefined;
  }
  return findBlockForRange(document, range);
}

function normalizeRangeForBlock(
  document: vscode.TextDocument,
  block: CodeBlock,
  range: vscode.Range
): vscode.Range {
  const startLine = Math.max(range.start.line, block.startLine);
  const endLine = Math.min(rangeTerminalLine(range), block.endLine);
  const startCharacter = startLine === range.start.line ? range.start.character : 0;
  const endCharacter = endLine === range.end.line
    ? range.end.character
    : document.lineAt(endLine).text.length;

  return new vscode.Range(startLine, startCharacter, endLine, endCharacter);
}

function blockSourceRange(
  document: vscode.TextDocument,
  block: CodeBlock
): vscode.Range {
  return new vscode.Range(
    block.startLine,
    0,
    block.endLine,
    document.lineAt(block.endLine).text.length
  );
}

function extractBlockContent(
  document: vscode.TextDocument,
  block: CodeBlock
): string {
  const lines: string[] = [];
  for (
    let i = block.startLine;
    i <= block.endLine && i < document.lineCount;
    i++
  ) {
    lines.push(document.lineAt(i).text);
  }
  return lines.join('\n');
}

// ============================================================
// Generated File LSP Bridge
// ============================================================

const RESOLVE_GENERATED_POSITION_COMMAND = 'mds.resolveGeneratedPosition';
const REMAP_GENERATED_LOCATIONS_COMMAND = 'mds.remapGeneratedLocations';
const REMAP_GENERATED_RANGE_COMMAND = 'mds.remapGeneratedRange';
const REMAP_GENERATED_TEXT_EDITS_COMMAND = 'mds.remapGeneratedTextEdits';
const REMAP_GENERATED_TEXT_DOCUMENT_EDITS_COMMAND = 'mds.remapGeneratedTextDocumentEdits';

interface BridgePosition {
  line: number;
  character: number;
}

interface BridgeRange {
  start: BridgePosition;
  end: BridgePosition;
}

interface BridgeLocation {
  uri: string;
  range: BridgeRange;
}

interface BridgeTextEdit {
  range: BridgeRange;
  newText: string;
}

interface BridgeTextDocumentEdits {
  uri: string;
  edits: BridgeTextEdit[];
}

interface RemappedTextDocumentEdits {
  uri: vscode.Uri;
  edits: vscode.TextEdit[];
}

interface TextEditsByUriEntry {
  uri: vscode.Uri;
  edits: vscode.TextEdit[];
}

interface MirroredDiagnosticCandidate extends DiagnosticMirrorDiagnostic {
  original: vscode.Diagnostic;
}

const diagnosticMirrorState = createMirroredDiagnosticState<vscode.Diagnostic>();
let lastMirroredDiagnostics: readonly MirroredDiagnosticCollectionEntry<vscode.Diagnostic>[] = [];

let mirroredDiagnosticsCollection: vscode.DiagnosticCollection | undefined;

async function executeBridgeCommand<T>(
  command: string,
  args: unknown
): Promise<T | undefined> {
  if (!client) {
    return undefined;
  }

  try {
    const result = await client.sendRequest<T | null>('workspace/executeCommand', {
      command,
      arguments: [args],
    });
    return result ?? undefined;
  } catch {
    return undefined;
  }
}

function toBridgePosition(position: vscode.Position): BridgePosition {
  return {
    line: position.line,
    character: position.character,
  };
}

function fromBridgePosition(position: BridgePosition): vscode.Position {
  return new vscode.Position(position.line, position.character);
}

function toBridgeRange(range: vscode.Range): BridgeRange {
  return {
    start: toBridgePosition(range.start),
    end: toBridgePosition(range.end),
  };
}

function fromBridgeRange(range: BridgeRange): vscode.Range {
  return new vscode.Range(
    fromBridgePosition(range.start),
    fromBridgePosition(range.end)
  );
}

function toBridgeLocation(location: vscode.Location): BridgeLocation {
  return {
    uri: location.uri.toString(),
    range: toBridgeRange(location.range),
  };
}

function fromBridgeLocation(location: BridgeLocation): vscode.Location {
  return new vscode.Location(
    vscode.Uri.parse(location.uri),
    fromBridgeRange(location.range)
  );
}

function toBridgeTextEdit(edit: vscode.TextEdit): BridgeTextEdit {
  return {
    range: toBridgeRange(edit.range),
    newText: edit.newText,
  };
}

function fromBridgeTextEdit(edit: BridgeTextEdit): vscode.TextEdit {
  return new vscode.TextEdit(
    fromBridgeRange(edit.range),
    edit.newText
  );
}

function toBridgeTextDocumentEdits(
  uri: vscode.Uri,
  edits: readonly vscode.TextEdit[]
): BridgeTextDocumentEdits {
  return {
    uri: uri.toString(),
    edits: edits.map((edit) => toBridgeTextEdit(edit)),
  };
}

function fromBridgeTextDocumentEdits(
  document: BridgeTextDocumentEdits
): RemappedTextDocumentEdits {
  return {
    uri: vscode.Uri.parse(document.uri),
    edits: document.edits.map((edit) => fromBridgeTextEdit(edit)),
  };
}

async function resolveGeneratedPosition(
  markdownUri: vscode.Uri,
  position: vscode.Position
): Promise<vscode.Location | undefined> {
  const resolved = await executeBridgeCommand<BridgeLocation | null>(
    RESOLVE_GENERATED_POSITION_COMMAND,
    {
      markdown_uri: markdownUri.toString(),
      position: toBridgePosition(position),
    }
  );
  return resolved ? fromBridgeLocation(resolved) : undefined;
}

async function remapGeneratedRange(
  uri: vscode.Uri,
  range: vscode.Range
): Promise<vscode.Location | undefined> {
  const remapped = await executeBridgeCommand<BridgeLocation | null>(
    REMAP_GENERATED_RANGE_COMMAND,
    {
      uri: uri.toString(),
      range: toBridgeRange(range),
    }
  );
  return remapped ? fromBridgeLocation(remapped) : undefined;
}

async function remapGeneratedLocations(
  locations: readonly vscode.Location[]
): Promise<Array<vscode.Location | undefined> | undefined> {
  const remapped = await executeBridgeCommand<Array<BridgeLocation | null> | null>(
    REMAP_GENERATED_LOCATIONS_COMMAND,
    {
      locations: locations.map((location) => toBridgeLocation(location)),
    }
  );
  if (!remapped || remapped.length !== locations.length) {
    return undefined;
  }
  return remapped?.map((location) =>
    location ? fromBridgeLocation(location) : undefined
  );
}

async function remapGeneratedTextEdits(
  uri: vscode.Uri,
  edits: readonly vscode.TextEdit[]
): Promise<RemappedTextDocumentEdits | undefined> {
  const remapped = await executeBridgeCommand<BridgeTextDocumentEdits | null>(
    REMAP_GENERATED_TEXT_EDITS_COMMAND,
    toBridgeTextDocumentEdits(uri, edits)
  );
  return remapped ? fromBridgeTextDocumentEdits(remapped) : undefined;
}

async function remapGeneratedTextDocumentEdits(
  documents: readonly { uri: vscode.Uri; edits: readonly vscode.TextEdit[] }[]
): Promise<Array<RemappedTextDocumentEdits | undefined> | undefined> {
  const remapped = await executeBridgeCommand<Array<BridgeTextDocumentEdits | null> | null>(
    REMAP_GENERATED_TEXT_DOCUMENT_EDITS_COMMAND,
    {
      documents: documents.map((document) =>
        toBridgeTextDocumentEdits(document.uri, document.edits)
      ),
    }
  );
  if (!remapped || remapped.length !== documents.length) {
    return undefined;
  }
  return remapped?.map((document) =>
    document ? fromBridgeTextDocumentEdits(document) : undefined
  );
}

function isDefinitionLink(
  value: vscode.Location | vscode.DefinitionLink
): value is vscode.DefinitionLink {
  return 'targetUri' in value;
}

function definitionTargetsToLocations(
  definitions: vscode.Location[] | vscode.DefinitionLink[] | undefined
): vscode.Location[] {
  if (!definitions) {
    return [];
  }

  return definitions.map((definition) =>
    isDefinitionLink(definition)
      ? new vscode.Location(
        definition.targetUri,
        definition.targetSelectionRange || definition.targetRange
      )
      : definition
  );
}

async function openGeneratedDocument(
  markdownUri: vscode.Uri,
  position: vscode.Position
): Promise<{ document: vscode.TextDocument; position: vscode.Position } | undefined> {
  const resolved = await resolveGeneratedPosition(markdownUri, position);
  if (!resolved) {
    return undefined;
  }

  try {
    const document = await vscode.workspace.openTextDocument(resolved.uri);
    return {
      document,
      position: resolved.range.start,
    };
  } catch {
    return undefined;
  }
}

async function openGeneratedDocumentForRange(
  markdownUri: vscode.Uri,
  range: vscode.Range
): Promise<{ document: vscode.TextDocument; range: vscode.Range } | undefined> {
  const [resolvedStart, resolvedEnd] = await Promise.all([
    resolveGeneratedPosition(markdownUri, range.start),
    resolveGeneratedPosition(markdownUri, range.end),
  ]);
  if (!resolvedStart || !resolvedEnd) {
    return undefined;
  }
  if (resolvedStart.uri.toString() !== resolvedEnd.uri.toString()) {
    return undefined;
  }

  try {
    const document = await vscode.workspace.openTextDocument(resolvedStart.uri);
    return {
      document,
      range: new vscode.Range(
        resolvedStart.range.start,
        resolvedEnd.range.start
      ),
    };
  } catch {
    return undefined;
  }
}

// ============================================================
// Embedded Language Support
// ============================================================

/**
 * Cache for shadow documents used for embedded language delegation.
 * Key: "{sourceUri}#{blockIndex}", Value: { content, doc }
 */
interface ShadowCacheEntry extends ShadowDocumentIdentity {
  doc: vscode.TextDocument;
}

const shadowCache = new Map<
  string,
  ShadowCacheEntry
>();

let embeddedProvider: EmbeddedDocumentProvider | undefined;

function shadowCacheKey(sourceUri: string, blockIndex: number): string {
  return `${sourceUri}#${blockIndex}`;
}

/**
 * Get or create a shadow document for a code block.
 * Shadow documents are untitled documents with the appropriate language ID,
 * allowing VS Code's built-in language services to provide features.
 */
async function getOrCreateShadowDoc(
  document: vscode.TextDocument,
  block: CodeBlock
): Promise<vscode.TextDocument | undefined> {
  const key = shadowCacheKey(document.uri.toString(), block.index);
  const content = extractBlockContent(document, block);
  const shadowUri = embeddedBlockUri(document, block);
  const requestedIdentity: ShadowDocumentIdentity = {
    content,
    uri: toDiagnosticMirrorUri(shadowUri),
  };

  const cached = shadowCache.get(key);
  if (
    cached
    && !cached.doc.isClosed
    && shadowDocumentIdentityEqual(cached, requestedIdentity)
  ) {
    return cached.doc;
  }

  try {
    const doc = await vscode.workspace.openTextDocument(shadowUri);
    if (doc.languageId !== block.languageId) {
      await vscode.languages.setTextDocumentLanguage(doc, block.languageId);
    }
    shadowCache.set(key, { ...requestedIdentity, doc });
    return doc;
  } catch {
    return undefined;
  }
}

function embeddedBlockUri(
  document: vscode.TextDocument,
  block: CodeBlock
): vscode.Uri {
  const sourcePath = document.uri.path.replace(/\.md$/, '');
  return vscode.Uri.from({
    scheme: 'mds-embedded',
    authority: document.uri.authority,
    path: `${sourcePath}.block${block.index}${block.virtualExt}`,
    query: new URLSearchParams({
      source: document.uri.toString(),
      block: block.index.toString(),
      startLine: block.startLine.toString(),
      endLine: block.endLine.toString(),
    }).toString(),
  });
}

function parseEmbeddedSourceInfo(
  uri: vscode.Uri
): { sourceUri: vscode.Uri; startLine: number } | undefined {
  if (uri.scheme !== 'mds-embedded') {
    return undefined;
  }

  const sourceInfo = parseEmbeddedDiagnosticSourceInfo(uri.query);
  if (!sourceInfo) {
    return undefined;
  }

  try {
    return {
      sourceUri: vscode.Uri.parse(sourceInfo.source, true),
      startLine: sourceInfo.startLine,
    };
  } catch {
    return undefined;
  }
}

function sourcePositionFromEmbeddedStartLine(
  startLine: number,
  position: vscode.Position
): vscode.Position {
  return new vscode.Position(position.line + startLine, position.character);
}

function sourcePositionFromEmbedded(
  block: CodeBlock,
  position: vscode.Position
): vscode.Position {
  return sourcePositionFromEmbeddedStartLine(block.startLine, position);
}

function embeddedPositionFromSource(
  block: CodeBlock,
  position: vscode.Position
): vscode.Position {
  return new vscode.Position(position.line - block.startLine, position.character);
}

function sourceRangeFromEmbedded(
  block: CodeBlock,
  range: vscode.Range
): vscode.Range {
  return sourceRangeFromEmbeddedStartLine(block.startLine, range);
}

function sourceRangeFromEmbeddedStartLine(
  startLine: number,
  range: vscode.Range
): vscode.Range {
  return new vscode.Range(
    sourcePositionFromEmbeddedStartLine(startLine, range.start),
    sourcePositionFromEmbeddedStartLine(startLine, range.end)
  );
}

function embeddedRangeFromSource(
  block: CodeBlock,
  range: vscode.Range
): vscode.Range {
  return new vscode.Range(
    embeddedPositionFromSource(block, range.start),
    embeddedPositionFromSource(block, range.end)
  );
}

function mapEmbeddedLocationToSource(location: vscode.Location): vscode.Location {
  const sourceInfo = parseEmbeddedSourceInfo(location.uri);
  if (!sourceInfo) {
    return location;
  }
  return new vscode.Location(
    sourceInfo.sourceUri,
    sourceRangeFromEmbeddedStartLine(sourceInfo.startLine, location.range)
  );
}

function mapEmbeddedTextEditToSourceWithStartLine(
  startLine: number,
  edit: vscode.TextEdit
): vscode.TextEdit {
  return new vscode.TextEdit(
    sourceRangeFromEmbeddedStartLine(startLine, edit.range),
    edit.newText
  );
}

function mapEmbeddedTextEditToSource(
  block: CodeBlock,
  edit: vscode.TextEdit
): vscode.TextEdit {
  return mapEmbeddedTextEditToSourceWithStartLine(block.startLine, edit);
}

function mapEmbeddedTextEditsToSource(
  block: CodeBlock,
  edits: readonly vscode.TextEdit[]
): vscode.TextEdit[] {
  return edits.map((edit) => mapEmbeddedTextEditToSource(block, edit));
}

function appendTextEdits(
  editsByUri: Map<string, TextEditsByUriEntry>,
  uri: vscode.Uri,
  edits: readonly vscode.TextEdit[]
): void {
  if (edits.length === 0) {
    return;
  }

  const key = uri.toString();
  const existing = editsByUri.get(key);
  if (existing) {
    existing.edits.push(...edits);
    return;
  }

  editsByUri.set(key, {
    uri,
    edits: [...edits],
  });
}

function hasOnlyTextWorkspaceEditEntries(
  workspaceEdit: vscode.WorkspaceEdit
): boolean {
  return workspaceEdit.size === workspaceEdit.entries().length;
}

function mergeGeneratedLocations(
  remapped: readonly (vscode.Location | undefined)[]
): vscode.Location[] {
  return remapped.filter(
    (location): location is vscode.Location => !!location
  );
}

async function remapGeneratedWorkspaceEdit(
  workspaceEdit: vscode.WorkspaceEdit
): Promise<vscode.WorkspaceEdit | undefined> {
  if (!hasOnlyTextWorkspaceEditEntries(workspaceEdit)) {
    return undefined;
  }

  const entries = workspaceEdit.entries();
  if (entries.length === 0) {
    return workspaceEdit;
  }

  const remappedDocuments = await remapGeneratedTextDocumentEdits(
    entries.map(([uri, edits]) => ({ uri, edits }))
  );
  if (!remappedDocuments) {
    return undefined;
  }

  const editsByUri = new Map<string, TextEditsByUriEntry>();
  for (const remappedDocument of remappedDocuments) {
    if (!remappedDocument) {
      return undefined;
    }
    appendTextEdits(editsByUri, remappedDocument.uri, remappedDocument.edits);
  }

  const remappedWorkspaceEdit = new vscode.WorkspaceEdit();
  for (const { uri, edits } of editsByUri.values()) {
    remappedWorkspaceEdit.set(uri, edits);
  }

  return remappedWorkspaceEdit;
}

function mapEmbeddedWorkspaceEditToSource(
  workspaceEdit: vscode.WorkspaceEdit
): vscode.WorkspaceEdit | undefined {
  if (!hasOnlyTextWorkspaceEditEntries(workspaceEdit)) {
    return undefined;
  }

  const entries = workspaceEdit.entries();
  if (entries.length === 0) {
    return workspaceEdit;
  }

  const editsByUri = new Map<string, TextEditsByUriEntry>();
  for (const [uri, edits] of entries) {
    const sourceInfo = parseEmbeddedSourceInfo(uri);
    if (sourceInfo) {
      appendTextEdits(
        editsByUri,
        sourceInfo.sourceUri,
        edits.map((edit) =>
          mapEmbeddedTextEditToSourceWithStartLine(sourceInfo.startLine, edit)
        )
      );
      continue;
    }

    appendTextEdits(editsByUri, uri, edits);
  }

  const remappedWorkspaceEdit = new vscode.WorkspaceEdit();
  for (const { uri, edits } of editsByUri.values()) {
    remappedWorkspaceEdit.set(uri, edits);
  }

  return remappedWorkspaceEdit;
}

function isCodeAction(
  value: vscode.Command | vscode.CodeAction
): value is vscode.CodeAction {
  return 'edit' in value
    || 'kind' in value
    || 'diagnostics' in value
    || 'disabled' in value
    || 'isPreferred' in value;
}

function hasEditBackedCodeActionPayload(action: vscode.CodeAction): boolean {
  return !!action.edit;
}

function stripCodeActionCommand(action: vscode.CodeAction): vscode.CodeAction {
  action.command = undefined;
  return action;
}

async function remapGeneratedCodeAction(
  markdownUri: vscode.Uri,
  generatedUri: vscode.Uri,
  action: vscode.Command | vscode.CodeAction
): Promise<vscode.Command | vscode.CodeAction | undefined> {
  if (!isCodeAction(action)) {
    return undefined;
  }

  if (!hasEditBackedCodeActionPayload(action)) {
    return undefined;
  }

  if (action.edit) {
    const remappedEdit = await remapGeneratedWorkspaceEdit(action.edit);
    if (!remappedEdit) {
      return undefined;
    }
    action.edit = remappedEdit;
  }

  if (action.diagnostics) {
    const remappedDiagnostics = await Promise.all(
      action.diagnostics.map(async (diagnostic) => {
        const remapped = await remapGeneratedRange(generatedUri, diagnostic.range);
        if (!remapped || remapped.uri.toString() !== markdownUri.toString()) {
          return undefined;
        }
        return cloneDiagnosticForMarkdown(remapped.range, diagnostic, 'generated');
      })
    );
    action.diagnostics = remappedDiagnostics.filter(
      (diagnostic): diagnostic is vscode.Diagnostic => !!diagnostic
    );
  }

  return stripCodeActionCommand(action);
}

function mapEmbeddedCodeActionToSource(
  block: CodeBlock,
  action: vscode.Command | vscode.CodeAction
): vscode.Command | vscode.CodeAction | undefined {
  if (!isCodeAction(action)) {
    return undefined;
  }

  if (!hasEditBackedCodeActionPayload(action)) {
    return undefined;
  }

  if (action.edit) {
    const remappedEdit = mapEmbeddedWorkspaceEditToSource(action.edit);
    if (!remappedEdit) {
      return undefined;
    }
    action.edit = remappedEdit;
  }

  if (action.diagnostics) {
    action.diagnostics = action.diagnostics.map((diagnostic) =>
      cloneDiagnosticForMarkdown(
        sourceRangeFromEmbedded(block, diagnostic.range),
        diagnostic,
        'embedded'
      )
    );
  }

  return stripCodeActionCommand(action);
}

function mapCompletionItemToSource(
  block: CodeBlock,
  item: vscode.CompletionItem
): vscode.CompletionItem {
  const textEdit = item.textEdit;
  if (textEdit instanceof vscode.TextEdit) {
    item.textEdit = new vscode.TextEdit(
      sourceRangeFromEmbedded(block, textEdit.range),
      textEdit.newText
    );
  }
  item.additionalTextEdits = item.additionalTextEdits?.map(
    (edit) => new vscode.TextEdit(sourceRangeFromEmbedded(block, edit.range), edit.newText)
  );
  return item;
}

/**
 * Register embedded language feature providers for code blocks.
 * These providers detect when the cursor is inside a code block and
 * delegate to the appropriate language's built-in providers via
 * shadow documents.
 */
function registerEmbeddedLanguageProviders(
  context: vscode.ExtensionContext,
  runtimeState: ExtensionRuntimeState
): void {
  const selector: vscode.DocumentSelector = { language: 'mds-markdown' };

  // Completion provider for embedded code blocks
  context.subscriptions.push(
    vscode.languages.registerCompletionItemProvider(
      selector,
      {
        async provideCompletionItems(document, position, _token, context) {
          const block = findManagedBlockAtPosition(document, position.line, runtimeState.authoringRoots);
          if (!block) {
            return undefined;
          }

          const shadowDoc = await getOrCreateShadowDoc(document, block);
          if (!shadowDoc) {
            return undefined;
          }

          const virtualPos = new vscode.Position(
            position.line - block.startLine,
            position.character
          );

          try {
            const result =
              await vscode.commands.executeCommand<vscode.CompletionList>(
                'vscode.executeCompletionItemProvider',
                shadowDoc.uri,
                virtualPos,
                context.triggerCharacter
              );
            if (!result) {
              return undefined;
            }
            result.items = result.items.map((item) =>
              mapCompletionItemToSource(block, item)
            );
            return result;
          } catch {
            return undefined;
          }
        },
      },
      '.',
      ':',
      '('
    )
  );

  // Hover provider for embedded code blocks
  context.subscriptions.push(
    vscode.languages.registerHoverProvider(selector, {
      async provideHover(document, position) {
        const block = findManagedBlockAtPosition(document, position.line, runtimeState.authoringRoots);
        if (!block) {
          return undefined;
        }

        try {
          const generated = await openGeneratedDocument(document.uri, position);
          if (generated) {
            const hovers = await vscode.commands.executeCommand<vscode.Hover[]>(
              'vscode.executeHoverProvider',
              generated.document.uri,
              generated.position
            );
            const hover = hovers?.[0];
            if (hover) {
              if (!hover.range) {
                return hover;
              }

              const remapped = await remapGeneratedRange(
                generated.document.uri,
                hover.range
              );
              if (remapped?.uri.toString() === document.uri.toString()) {
                return new vscode.Hover(hover.contents, remapped.range);
              }
              return new vscode.Hover(hover.contents);
            }
          }
        } catch {
          // Fall through to shadow document fallback.
        }

        const shadowDoc = await getOrCreateShadowDoc(document, block);
        if (!shadowDoc) {
          return undefined;
        }

        const virtualPos = new vscode.Position(
          position.line - block.startLine,
          position.character
        );

        try {
          const hovers = await vscode.commands.executeCommand<vscode.Hover[]>(
            'vscode.executeHoverProvider',
            shadowDoc.uri,
            virtualPos
          );
          if (hovers && hovers.length > 0) {
            const hover = hovers[0];
            if (!hover.range) {
              return hover;
            }
            return new vscode.Hover(
              hover.contents,
              sourceRangeFromEmbedded(block, hover.range)
            );
          }
        } catch {
          // Fall through
        }
        return undefined;
      },
    })
  );

  // Definition provider for embedded code blocks
  context.subscriptions.push(
    vscode.languages.registerDefinitionProvider(selector, {
      async provideDefinition(document, position) {
        const block = findManagedBlockAtPosition(document, position.line, runtimeState.authoringRoots);
        if (!block) {
          return undefined;
        }

        try {
          const generated = await openGeneratedDocument(document.uri, position);
          if (generated) {
            const definitions =
              await vscode.commands.executeCommand<
                vscode.Location[] | vscode.DefinitionLink[]
              >(
                'vscode.executeDefinitionProvider',
                generated.document.uri,
                generated.position
              );
            const generatedLocations = definitionTargetsToLocations(definitions);
            if (generatedLocations.length > 0) {
              const remapped = await remapGeneratedLocations(generatedLocations);
              if (remapped) {
                const mappedLocations = mergeGeneratedLocations(remapped);
                if (mappedLocations.length > 0) {
                  return mappedLocations;
                }
              }
            }
          }
        } catch {
          // Fall through to shadow document fallback.
        }

        const shadowDoc = await getOrCreateShadowDoc(document, block);
        if (!shadowDoc) {
          return undefined;
        }

        const virtualPos = new vscode.Position(
          position.line - block.startLine,
          position.character
        );

        try {
          const locations =
            await vscode.commands.executeCommand<vscode.Location[]>(
              'vscode.executeDefinitionProvider',
              shadowDoc.uri,
              virtualPos
            );
          return locations?.map((location) => mapEmbeddedLocationToSource(location)) || undefined;
        } catch {
          return undefined;
        }
      },
    })
  );

  // References provider for embedded code blocks
  context.subscriptions.push(
    vscode.languages.registerReferenceProvider(selector, {
      async provideReferences(document, position) {
        const block = findManagedBlockAtPosition(document, position.line, runtimeState.authoringRoots);
        if (!block) {
          return undefined;
        }

        try {
          const generated = await openGeneratedDocument(document.uri, position);
          if (generated) {
            const locations = await vscode.commands.executeCommand<vscode.Location[]>(
              'vscode.executeReferenceProvider',
              generated.document.uri,
              generated.position
            );
            if (locations && locations.length > 0) {
              const remapped = await remapGeneratedLocations(locations);
              if (remapped) {
                const mappedLocations = mergeGeneratedLocations(remapped);
                if (mappedLocations.length > 0) {
                  return mappedLocations;
                }
              }
            }
          }
        } catch {
          // Fall through to shadow document fallback.
        }

        const shadowDoc = await getOrCreateShadowDoc(document, block);
        if (!shadowDoc) {
          return undefined;
        }

        const virtualPos = new vscode.Position(
          position.line - block.startLine,
          position.character
        );

        try {
          const locations = await vscode.commands.executeCommand<vscode.Location[]>(
            'vscode.executeReferenceProvider',
            shadowDoc.uri,
            virtualPos
          );
          return locations?.map((location) => mapEmbeddedLocationToSource(location)) || undefined;
        } catch {
          return undefined;
        }
      },
    })
  );

  // Rename provider for embedded code blocks
  context.subscriptions.push(
    vscode.languages.registerRenameProvider(selector, {
      async provideRenameEdits(document, position, newName) {
        const block = findManagedBlockAtPosition(document, position.line, runtimeState.authoringRoots);
        if (!block) {
          return undefined;
        }

        try {
          const generated = await openGeneratedDocument(document.uri, position);
          if (generated) {
            const workspaceEdit = await vscode.commands.executeCommand<vscode.WorkspaceEdit>(
              'vscode.executeDocumentRenameProvider',
              generated.document.uri,
              generated.position,
              newName
            );
            if (workspaceEdit) {
              const remapped = await remapGeneratedWorkspaceEdit(workspaceEdit);
              if (remapped) {
                return remapped;
              }
            }
          }
        } catch {
          // Fall through to shadow document fallback.
        }

        const shadowDoc = await getOrCreateShadowDoc(document, block);
        if (!shadowDoc) {
          return undefined;
        }

        const virtualPos = new vscode.Position(
          position.line - block.startLine,
          position.character
        );

        try {
          const workspaceEdit = await vscode.commands.executeCommand<vscode.WorkspaceEdit>(
            'vscode.executeDocumentRenameProvider',
            shadowDoc.uri,
            virtualPos,
            newName
          );
          return workspaceEdit ? mapEmbeddedWorkspaceEditToSource(workspaceEdit) : undefined;
        } catch {
          return undefined;
        }
      },
    })
  );

  // Code action provider for embedded code blocks
  context.subscriptions.push(
    vscode.languages.registerCodeActionsProvider(selector, {
      async provideCodeActions(document, range, context) {
        const block = findManagedBlockForRange(document, range, runtimeState.authoringRoots);
        if (!block) {
          return undefined;
        }

        const normalizedRange = normalizeRangeForBlock(document, block, range);

        try {
          const generated = await openGeneratedDocumentForRange(
            document.uri,
            normalizedRange
          );
          if (generated) {
            const actions = await vscode.commands.executeCommand<
              Array<vscode.Command | vscode.CodeAction>
            >(
              'vscode.executeCodeActionProvider',
              generated.document.uri,
              generated.range,
              context.only,
              16
            );
            if (actions && actions.length > 0) {
              const remappedActions = (await Promise.all(
                actions.map((action) =>
                  remapGeneratedCodeAction(
                    document.uri,
                    generated.document.uri,
                    action
                  )
                )
              )).filter(
                (action): action is vscode.Command | vscode.CodeAction => !!action
              );
              if (remappedActions.length > 0) {
                return remappedActions;
              }
            }
          }
        } catch {
          // Fall through to shadow document fallback.
        }

        const shadowDoc = await getOrCreateShadowDoc(document, block);
        if (!shadowDoc) {
          return undefined;
        }

        try {
          const actions = await vscode.commands.executeCommand<
            Array<vscode.Command | vscode.CodeAction>
          >(
            'vscode.executeCodeActionProvider',
            shadowDoc.uri,
            embeddedRangeFromSource(block, normalizedRange),
            context.only,
            16
          );
          if (!actions || actions.length === 0) {
            return undefined;
          }
          const remappedActions = actions.flatMap((action) => {
            const remapped = mapEmbeddedCodeActionToSource(block, action);
            return remapped ? [remapped] : [];
          });
          return remappedActions.length > 0 ? remappedActions : undefined;
        } catch {
          return undefined;
        }
      },
    })
  );

  // Document range formatting provider for embedded code blocks
  context.subscriptions.push(
    vscode.languages.registerDocumentRangeFormattingEditProvider(selector, {
      async provideDocumentRangeFormattingEdits(document, range, options) {
        const block = findManagedBlockForRange(document, range, runtimeState.authoringRoots);
        if (!block) {
          return undefined;
        }

        const normalizedRange = normalizeRangeForBlock(document, block, range);

        try {
          const generated = await openGeneratedDocumentForRange(
            document.uri,
            normalizedRange
          );
          if (generated) {
            const edits = await vscode.commands.executeCommand<vscode.TextEdit[]>(
              'vscode.executeFormatRangeProvider',
              generated.document.uri,
              generated.range,
              options
            );
            if (edits && edits.length > 0) {
              const remapped = await remapGeneratedTextEdits(
                generated.document.uri,
                edits
              );
              if (remapped?.uri.toString() === document.uri.toString()) {
                return remapped.edits;
              }
            }
          }
        } catch {
          // Fall through to shadow document fallback.
        }

        const shadowDoc = await getOrCreateShadowDoc(document, block);
        if (!shadowDoc) {
          return undefined;
        }

        try {
          const edits = await vscode.commands.executeCommand<vscode.TextEdit[]>(
            'vscode.executeFormatRangeProvider',
            shadowDoc.uri,
            embeddedRangeFromSource(block, normalizedRange),
            options
          );
          return edits && edits.length > 0
            ? mapEmbeddedTextEditsToSource(block, edits)
            : undefined;
        } catch {
          return undefined;
        }
      },
    })
  );

  // Document formatting provider for embedded code blocks
  context.subscriptions.push(
    vscode.languages.registerDocumentFormattingEditProvider(selector, {
      async provideDocumentFormattingEdits(document, options) {
        if (!isManagedMarkdownAuthoringDocument(document, runtimeState.authoringRoots)) {
          return undefined;
        }

        const blocks = getCodeBlocks(document);
        if (blocks.length === 0) {
          return undefined;
        }

        if (blocks.length === 1) {
          try {
            const generated = await openGeneratedDocumentForRange(
              document.uri,
              blockSourceRange(document, blocks[0])
            );
            if (generated) {
              const edits = await vscode.commands.executeCommand<vscode.TextEdit[]>(
                'vscode.executeFormatDocumentProvider',
                generated.document.uri,
                options
              );
              if (edits && edits.length > 0) {
                const remapped = await remapGeneratedTextEdits(
                  generated.document.uri,
                  edits
                );
                if (remapped?.uri.toString() === document.uri.toString()) {
                  return remapped.edits;
                }
              }
            }
          } catch {
            // Fall through to shadow document formatting.
          }
        }

        const allEdits: vscode.TextEdit[] = [];
        for (const block of blocks) {
          const shadowDoc = await getOrCreateShadowDoc(document, block);
          if (!shadowDoc) {
            continue;
          }

          try {
            const edits = await vscode.commands.executeCommand<vscode.TextEdit[]>(
              'vscode.executeFormatDocumentProvider',
              shadowDoc.uri,
              options
            );
            if (edits && edits.length > 0) {
              allEdits.push(...mapEmbeddedTextEditsToSource(block, edits));
            }
          } catch {
            // Continue with remaining blocks.
          }
        }

        return allEdits.length > 0 ? allEdits : undefined;
      },
    })
  );
}

// ============================================================
// Generated / Embedded Diagnostics Mirror
// ============================================================

function toDiagnosticMirrorUri(uri: vscode.Uri): DiagnosticMirrorUri {
  return {
    scheme: uri.scheme,
    path: uri.path,
    query: uri.query,
  };
}

function fromDiagnosticMirrorUri(uri: DiagnosticMirrorUri): vscode.Uri {
  return vscode.Uri.from({
    scheme: uri.scheme,
    path: uri.path,
    query: uri.query || '',
  });
}

function toDiagnosticMirrorDiagnostic(
  diagnostic: vscode.Diagnostic
): MirroredDiagnosticCandidate {
  return {
    range: toBridgeRange(diagnostic.range),
    message: diagnostic.message,
    severity: diagnostic.severity,
    code: diagnostic.code,
    source: diagnostic.source,
    tags: diagnostic.tags,
    original: diagnostic,
  };
}

function cloneDiagnosticForMarkdown(
  range: vscode.Range,
  diagnostic: vscode.Diagnostic,
  fallbackSource: MirroredDiagnosticUriKind
): vscode.Diagnostic {
  const mirrored = new vscode.Diagnostic(
    range,
    diagnostic.message,
    diagnostic.severity
  );
  mirrored.code = diagnostic.code;
  mirrored.source = diagnostic.source || fallbackSource;
  mirrored.tags = diagnostic.tags ? [...diagnostic.tags] : undefined;
  return mirrored;
}

function isActiveShadowDocumentUri(uri: vscode.Uri): boolean {
  const uriText = uri.toString();
  for (const cached of shadowCache.values()) {
    if (cached.doc.uri.toString() === uriText) {
      return true;
    }
  }

  return false;
}

function isMirroredDiagnosticEventUri(uri: vscode.Uri): boolean {
  const uriKind = classifyMirroredDiagnosticUri(toDiagnosticMirrorUri(uri));
  if (uriKind === 'generated') {
    return true;
  }

  if (uriKind === 'embedded') {
    return isActiveShadowDocumentUri(uri);
  }

  return false;
}

function sameDiagnosticCode(
  left: vscode.Diagnostic['code'],
  right: vscode.Diagnostic['code']
): boolean {
  if (left === right) {
    return true;
  }

  if (typeof left === 'object' || typeof right === 'object') {
    if (
      typeof left !== 'object' || !left
      || typeof right !== 'object' || !right
    ) {
      return false;
    }

    return left.value === right.value
      && left.target.toString() === right.target.toString();
  }

  return false;
}

function sameDiagnosticTags(
  left: readonly vscode.DiagnosticTag[] | undefined,
  right: readonly vscode.DiagnosticTag[] | undefined
): boolean {
  if (!left && !right) {
    return true;
  }

  if (!left || !right || left.length !== right.length) {
    return false;
  }

  return left.every((tag, index) => tag === right[index]);
}

function sameMirroredDiagnostic(
  left: vscode.Diagnostic,
  right: vscode.Diagnostic
): boolean {
  return left.message === right.message
    && left.severity === right.severity
    && left.source === right.source
    && sameDiagnosticCode(left.code, right.code)
    && sameDiagnosticTags(left.tags, right.tags)
    && left.range.isEqual(right.range);
}

function applyMirroredDiagnosticAggregate(
  aggregate: readonly MirroredDiagnosticCollectionEntry<vscode.Diagnostic>[]
): void {
  if (!mirroredDiagnosticsCollection) {
    return;
  }

  if (mirroredDiagnosticCollectionsEqual(
    lastMirroredDiagnostics,
    aggregate,
    sameMirroredDiagnostic
  )) {
    return;
  }

  lastMirroredDiagnostics = aggregate;

  mirroredDiagnosticsCollection.clear();
  if (aggregate.length === 0) {
    return;
  }

  mirroredDiagnosticsCollection.set(
    aggregate.map((entry) =>
      [
        vscode.Uri.parse(entry.uri, true),
        entry.diagnostics,
      ] as [vscode.Uri, vscode.Diagnostic[]]
    )
  );
}

function purgeMirroredDiagnostics(cacheKeys: readonly string[]): void {
  if (cacheKeys.length === 0) {
    return;
  }

  applyMirroredDiagnosticAggregate(
    purgeMirroredDiagnosticCacheKeys(diagnosticMirrorState, cacheKeys)
  );
}

async function refreshMirroredDiagnostics(
  uris: readonly vscode.Uri[]
): Promise<void> {
  if (!mirroredDiagnosticsCollection) {
    return;
  }

  const aggregate = await applyMirroredDiagnosticEvents(
    diagnosticMirrorState,
    uris.map((uri) => ({
      cacheKey: uri.toString(),
      uri: toDiagnosticMirrorUri(uri),
      diagnostics: vscode.languages.getDiagnostics(uri).map((diagnostic) =>
        toDiagnosticMirrorDiagnostic(diagnostic)
      ),
    })),
    {
      remapGeneratedRange: async (mirrorUri, range) => {
        const location = await remapGeneratedRange(
          fromDiagnosticMirrorUri(mirrorUri),
          fromBridgeRange(range)
        );
        if (!location) {
          return undefined;
        }

        return {
          uri: location.uri.toString(),
          range: toBridgeRange(location.range),
        };
      },
      createDiagnostic: ({ location, diagnostic, fallbackSource }) =>
        cloneDiagnosticForMarkdown(
          fromBridgeRange(location.range),
          diagnostic.original,
          fallbackSource
        ),
    }
  );

  applyMirroredDiagnosticAggregate(aggregate);
}

function syncShadowDocumentsForChangedSource(
  document: vscode.TextDocument
): void {
  const sourceUri = document.uri.toString();
  const nextBlocksByIndex = new Map(
    getCodeBlocks(document).map((block) => [block.index, block] as const)
  );
  const staleShadowUris: string[] = [];

  for (const [key, cached] of [...shadowCache.entries()]) {
    if (!key.startsWith(`${sourceUri}#`)) {
      continue;
    }

    const blockIndex = Number.parseInt(key.slice(sourceUri.length + 1), 10);
    const nextBlock = Number.isInteger(blockIndex)
      ? nextBlocksByIndex.get(blockIndex)
      : undefined;

    if (!nextBlock) {
      staleShadowUris.push(cached.doc.uri.toString());
      shadowCache.delete(key);
      continue;
    }

    const nextShadowUri = embeddedBlockUri(document, nextBlock);
    if (cached.doc.uri.toString() !== nextShadowUri.toString()) {
      staleShadowUris.push(cached.doc.uri.toString());
      shadowCache.delete(key);
      continue;
    }

    embeddedProvider?.refresh(cached.doc.uri);
    shadowCache.set(key, {
      content: extractBlockContent(document, nextBlock),
      uri: toDiagnosticMirrorUri(nextShadowUri),
      doc: cached.doc,
    });
  }

  purgeMirroredDiagnostics(staleShadowUris);
}

function purgeShadowDocumentsForSource(sourceUri: string): void {
  const staleShadowUris: string[] = [];

  for (const [key, cached] of [...shadowCache.entries()]) {
    if (!key.startsWith(`${sourceUri}#`)) {
      continue;
    }

    staleShadowUris.push(cached.doc.uri.toString());
    shadowCache.delete(key);
  }

  purgeMirroredDiagnostics(staleShadowUris);
}

function registerMirroredDiagnostics(
  context: vscode.ExtensionContext
): void {
  mirroredDiagnosticsCollection = vscode.languages.createDiagnosticCollection(
    'mds-diagnostic-mirror'
  );
  lastMirroredDiagnostics = [];

  context.subscriptions.push(
    mirroredDiagnosticsCollection,
    vscode.languages.onDidChangeDiagnostics((event) => {
      const mirroredUris = event.uris.filter((uri) => isMirroredDiagnosticEventUri(uri));
      if (mirroredUris.length === 0) {
        return;
      }

      void refreshMirroredDiagnostics(mirroredUris);
    })
  );

  void refreshMirroredDiagnostics(
    vscode.languages.getDiagnostics()
      .map(([uri]) => uri)
      .filter((uri) => isMirroredDiagnosticEventUri(uri))
  );
}

/**
 * Register the virtual document content provider for the mds-embedded scheme.
 * This provides document content for embedded code block URIs.
 */
function registerVirtualDocumentProvider(
  context: vscode.ExtensionContext
): void {
  const scheme = 'mds-embedded';

  embeddedProvider = new (class implements EmbeddedDocumentProvider {
    private readonly onDidChangeEmitter = new vscode.EventEmitter<vscode.Uri>();
    readonly onDidChange = this.onDidChangeEmitter.event;

    refresh(uri: vscode.Uri): void {
      this.onDidChangeEmitter.fire(uri);
    }

    provideTextDocumentContent(uri: vscode.Uri): string {
      const params = new URLSearchParams(uri.query);
      const sourceUriStr = params.get('source');
      const startLine = parseInt(params.get('startLine') || '0', 10);
      const endLine = parseInt(params.get('endLine') || '0', 10);

      if (!sourceUriStr) {
        return '';
      }

      const sourceUri = vscode.Uri.parse(sourceUriStr);
      const doc = vscode.workspace.textDocuments.find(
        (d) => d.uri.toString() === sourceUri.toString()
      );
      if (!doc) {
        return '';
      }

      const lines: string[] = [];
      for (let i = startLine; i <= endLine && i < doc.lineCount; i++) {
        lines.push(doc.lineAt(i).text);
      }
      return lines.join('\n');
    }
  })();

  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider(scheme, embeddedProvider)
  );
}

function registerPreviewCommands(context: vscode.ExtensionContext): void {
  async function preview(command: string, uri?: vscode.Uri): Promise<void> {
    const target = uri || vscode.window.activeTextEditor?.document.uri;
    if (!target) {
      return;
    }
    await vscode.commands.executeCommand(command, target);
  }

  context.subscriptions.push(
    vscode.commands.registerCommand('mds.openPreview', (uri?: vscode.Uri) =>
      preview('markdown.showPreview', uri)
    ),
    vscode.commands.registerCommand('mds.openPreviewToSide', (uri?: vscode.Uri) =>
      preview('markdown.showPreviewToSide', uri)
    )
  );
}

// ============================================================
// mds File Detection
// ============================================================

/**
 * Determine if a file URI is an mds-managed file.
 * True only for Markdown authoring files under canonical package roots.
 */
function isMdsFile(
  uri: vscode.Uri,
  authoringRoots: AuthoringRoots
): boolean {
  return uri.scheme === 'file'
    && isManagedMarkdownAuthoringPath(uri.fsPath, authoringRoots);
}

async function associateOpenMdsDocuments(
  authoringRoots: AuthoringRoots
): Promise<void> {
  for (const doc of vscode.workspace.textDocuments) {
    if (
      doc.languageId === 'markdown'
      && isMdsFile(doc.uri, authoringRoots)
    ) {
      await vscode.languages.setTextDocumentLanguage(doc, 'mds-markdown');
    }
  }
}

async function refreshDiscoveredWorkspaceContext(
  config: vscode.WorkspaceConfiguration,
  runtimeState: ExtensionRuntimeState,
  statusBar?: ActiveContextStatusBarController
): Promise<void> {
  runtimeState.activeLanguages = await discoverLanguages(config);
  runtimeState.authoringRoots = await discoverAuthoringRoots();
  await associateOpenMdsDocuments(runtimeState.authoringRoots);
  statusBar?.refresh();
}

// ============================================================
// Activation
// ============================================================

export async function activate(context: vscode.ExtensionContext) {
  const config = vscode.workspace.getConfiguration('mds.lsp');
  if (!config.get<boolean>('enabled', true)) {
    return;
  }

  const serverPath = resolveServerPath(config, context.extensionPath);
  if (!serverPath) {
    vscode.window.showWarningMessage(
      'mds-lsp binary not found. Install with: cargo install --git https://github.com/owo-x-project/owox-mds mds-lsp, or set mds.lsp.path in settings.'
    );
    return;
  }

  // Discover active languages from config files and settings
  const runtimeState: ExtensionRuntimeState = {
    activeLanguages: await discoverLanguages(config),
    authoringRoots: await discoverAuthoringRoots(),
  };

  const logLevel = config.get<string>('logLevel', 'info');

  const serverOptions: ServerOptions = {
    command: serverPath,
    args: [],
    transport: TransportKind.stdio,
    options: {
      env: {
        ...process.env,
        RUST_LOG: `mds_lsp=${logLevel}`,
      },
    },
  };

  // Document selector for config and canonical authoring roots
  const documentSelector: {
    scheme: string;
    language?: string;
    pattern?: string;
  }[] = [
      { scheme: 'file', pattern: '**/mds.config.toml' },
      { scheme: 'file', pattern: '**/.mds/source/**/*.md' },
      { scheme: 'file', pattern: '**/.mds/test/**/*.md' },
    ];

  // File watchers for config and canonical authoring roots
  const configWatcher = vscode.workspace.createFileSystemWatcher('**/mds.config.toml');
  const sourceAuthoringWatcher = vscode.workspace.createFileSystemWatcher('**/.mds/source/**/*.md');
  const testAuthoringWatcher = vscode.workspace.createFileSystemWatcher('**/.mds/test/**/*.md');
  const descriptorWatcher = vscode.workspace.createFileSystemWatcher('**/.mds/descriptors/**/*.toml');
  const fileEvents = [
    configWatcher,
    sourceAuthoringWatcher,
    testAuthoringWatcher,
    descriptorWatcher,
  ];
  context.subscriptions.push(...fileEvents);

  const clientOptions: LanguageClientOptions = {
    documentSelector,
    synchronize: { fileEvents },
    outputChannel: vscode.window.createOutputChannel('mds Language Server'),
  };

  client = new LanguageClient(
    'mds-lsp',
    'mds Language Server',
    serverOptions,
    clientOptions
  );

  // Register embedded language support
  registerPreviewCommands(context);
  registerVirtualDocumentProvider(context);
  registerEmbeddedLanguageProviders(context, runtimeState);

  const activeContextStatusBar = registerActiveContextStatusBar(
    context,
    runtimeState
  );

  const refreshWorkspaceContext = () => {
    void refreshDiscoveredWorkspaceContext(config, runtimeState, activeContextStatusBar);
  };

  context.subscriptions.push(
    configWatcher.onDidCreate(refreshWorkspaceContext),
    configWatcher.onDidChange(refreshWorkspaceContext),
    configWatcher.onDidDelete(refreshWorkspaceContext),
    descriptorWatcher.onDidCreate(refreshWorkspaceContext),
    descriptorWatcher.onDidChange(refreshWorkspaceContext),
    descriptorWatcher.onDidDelete(refreshWorkspaceContext)
  );

  // Invalidate code block cache on document changes
  context.subscriptions.push(
    vscode.workspace.onDidChangeTextDocument((e) => {
      const uriStr = e.document.uri.toString();
      blockCache.delete(uriStr);
      syncShadowDocumentsForChangedSource(e.document);

      if (vscode.window.activeTextEditor?.document.uri.toString() === uriStr) {
        activeContextStatusBar.refresh(vscode.window.activeTextEditor);
      }
    }),
    vscode.workspace.onDidCloseTextDocument((doc) => {
      const uriStr = doc.uri.toString();
      blockCache.delete(uriStr);
      purgeShadowDocumentsForSource(uriStr);
    })
  );

  // Watch for config changes to detect new language extensions
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration('mds.lsp')) {
        refreshWorkspaceContext();
        vscode.window
          .showInformationMessage(
            'mds LSP configuration changed. Restart to apply.',
            'Restart'
          )
          .then((selection) => {
            if (selection === 'Restart') {
              vscode.commands.executeCommand('workbench.action.reloadWindow');
            }
          });
      }
    })
  );

  // Auto-associate .md files in mds authoring roots with mds-markdown language
  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((doc) => {
      if (
        doc.languageId === 'markdown'
        && isMdsFile(doc.uri, runtimeState.authoringRoots)
      ) {
        vscode.languages.setTextDocumentLanguage(doc, 'mds-markdown');
      }
    })
  );

  // Set language for already-open documents
  await associateOpenMdsDocuments(runtimeState.authoringRoots);

  await client.start();
  registerMirroredDiagnostics(context);
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
  }
}

function resolveServerPath(
  config: vscode.WorkspaceConfiguration,
  extensionPath: string
): string | undefined {
  const configPath = config.get<string>('path', '');
  if (configPath) {
    return configPath;
  }

  const { execFileSync } = require('child_process');
  const path = require('path');
  const fs = require('fs');

  const platformKey = bundledPlatformKey();
  if (platformKey) {
    const bundled = path.join(
      extensionPath,
      'server',
      platformKey,
      process.platform === 'win32' ? 'mds-lsp.exe' : 'mds-lsp'
    );
    if (fs.existsSync(bundled)) {
      return bundled;
    }
  }

  // Try to find mds-lsp in PATH
  try {
    const cmd = process.platform === 'win32' ? 'where' : 'which';
    const result = execFileSync(cmd, ['mds-lsp'], {
      encoding: 'utf-8',
      timeout: 5000,
    }).trim();
    if (result) {
      return result.split('\n')[0].trim();
    }
  } catch {
    // Not found in PATH
  }

  // Try to find mds-lsp next to the mds binary
  try {
    const cmd = process.platform === 'win32' ? 'where' : 'which';
    const mdsPath = execFileSync(cmd, ['mds'], {
      encoding: 'utf-8',
      timeout: 5000,
    }).trim().split('\n')[0].trim();
    if (mdsPath) {
      const lspPath = path.join(path.dirname(mdsPath), 'mds-lsp');
      const lspPathExt = process.platform === 'win32' ? lspPath + '.exe' : lspPath;
      if (fs.existsSync(lspPathExt)) {
        return lspPathExt;
      }
    }
  } catch {
    // mds not found in PATH either
  }

  // Try common cargo install location
  const home = process.env.HOME || process.env.USERPROFILE || '';
  if (home) {
    const cargoLsp = path.join(home, '.cargo', 'bin', 'mds-lsp');
    const cargoLspExt = process.platform === 'win32' ? cargoLsp + '.exe' : cargoLsp;
    if (fs.existsSync(cargoLspExt)) {
      return cargoLspExt;
    }
  }

  return undefined;
}

function bundledPlatformKey(): string | undefined {
  if (process.platform === 'linux' && process.arch === 'x64') {
    return 'linux-x64';
  }
  if (process.platform === 'darwin' && process.arch === 'arm64') {
    return 'darwin-arm64';
  }
  if (process.platform === 'win32' && process.arch === 'x64') {
    return 'win32-x64';
  }
  return undefined;
}
