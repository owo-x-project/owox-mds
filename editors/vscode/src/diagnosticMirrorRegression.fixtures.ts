import type {
  DiagnosticMirrorDiagnostic,
  DiagnosticMirrorRange,
  MirroredDiagnosticCollectionEntry,
} from './diagnosticMirror';

export interface DiagnosticMirrorRegressionEvent {
  uri: string;
  diagnostics: readonly DiagnosticMirrorDiagnostic[];
}

export interface DiagnosticMirrorRegressionStep {
  name: string;
  events: readonly DiagnosticMirrorRegressionEvent[];
  purgeCacheKeys?: readonly string[];
  generatedRemaps?: Readonly<Record<string, string | null>>;
  expectedCollection: readonly MirroredDiagnosticCollectionEntry<DiagnosticMirrorDiagnostic>[];
}

export interface DiagnosticMirrorRegressionFixture {
  name: string;
  steps: readonly DiagnosticMirrorRegressionStep[];
}

export const SOURCE_URI = 'file:///workspace/examples/broken-remap-ts/.mds/source/foo/source-map.ts.md';
export const GENERATED_URI = 'file:///workspace/examples/broken-remap-ts/src/foo/source-map.ts';
export const EMBEDDED_URI = `mds-embedded:/workspace/examples/broken-remap-ts/.mds/source/foo/source-map.block0.ts?${new URLSearchParams({
  block: '0',
  source: SOURCE_URI,
  startLine: '13',
  endLine: '13',
}).toString()}`;
export const EMBEDDED_MOVED_URI = `mds-embedded:/workspace/examples/broken-remap-ts/.mds/source/foo/source-map.block0.ts?${new URLSearchParams({
  block: '0',
  source: SOURCE_URI,
  startLine: '23',
  endLine: '25',
}).toString()}`;

function range(
  startLine: number,
  startCharacter: number,
  endLine: number,
  endCharacter: number
): DiagnosticMirrorRange {
  return {
    start: {
      line: startLine,
      character: startCharacter,
    },
    end: {
      line: endLine,
      character: endCharacter,
    },
  };
}

export const diagnosticMirrorRegressionFixtures: readonly DiagnosticMirrorRegressionFixture[] = [
  {
    name: 'generated success',
    steps: [
      {
        name: 'generated diagnostics populate Markdown collection payload',
        events: [
          {
            uri: GENERATED_URI,
            diagnostics: [
              {
                message: 'type mismatch',
                range: range(1, 2, 3, 4),
                source: 'tsserver',
              },
              {
                message: 'implicit any',
                range: range(6, 1, 6, 8),
              },
            ],
          },
        ],
        generatedRemaps: {
          [GENERATED_URI]: SOURCE_URI,
        },
        expectedCollection: [
          {
            uri: SOURCE_URI,
            diagnostics: [
              {
                message: 'type mismatch',
                range: range(1, 2, 3, 4),
                source: 'tsserver',
              },
              {
                message: 'implicit any',
                range: range(6, 1, 6, 8),
                source: 'generated',
              },
            ],
          },
        ],
      },
    ],
  },
  {
    name: 'embedded success',
    steps: [
      {
        name: 'generated diagnostics seed existing Markdown cache',
        events: [
          {
            uri: GENERATED_URI,
            diagnostics: [
              {
                message: 'type mismatch',
                range: range(1, 2, 3, 4),
                source: 'tsserver',
              },
            ],
          },
        ],
        generatedRemaps: {
          [GENERATED_URI]: SOURCE_URI,
        },
        expectedCollection: [
          {
            uri: SOURCE_URI,
            diagnostics: [
              {
                message: 'type mismatch',
                range: range(1, 2, 3, 4),
                source: 'tsserver',
              },
            ],
          },
        ],
      },
      {
        name: 'embedded diagnostics merge into existing Markdown payload',
        events: [
          {
            uri: EMBEDDED_URI,
            diagnostics: [
              {
                message: 'unused binding',
                range: range(1, 2, 3, 4),
              },
            ],
          },
        ],
        expectedCollection: [
          {
            uri: SOURCE_URI,
            diagnostics: [
              {
                message: 'type mismatch',
                range: range(1, 2, 3, 4),
                source: 'tsserver',
              },
              {
                message: 'unused binding',
                range: range(14, 2, 16, 4),
                source: 'embedded',
              },
            ],
          },
        ],
      },
    ],
  },
  {
    name: 'unmanaged no-op',
    steps: [
      {
        name: 'generated diagnostics seed cache before unmanaged event',
        events: [
          {
            uri: GENERATED_URI,
            diagnostics: [
              {
                message: 'type mismatch',
                range: range(1, 2, 3, 4),
                source: 'tsserver',
              },
            ],
          },
        ],
        generatedRemaps: {
          [GENERATED_URI]: SOURCE_URI,
        },
        expectedCollection: [
          {
            uri: SOURCE_URI,
            diagnostics: [
              {
                message: 'type mismatch',
                range: range(1, 2, 3, 4),
                source: 'tsserver',
              },
            ],
          },
        ],
      },
      {
        name: 'unmanaged Markdown diagnostics are ignored and keep cached aggregate stable',
        events: [
          {
            uri: SOURCE_URI,
            diagnostics: [
              {
                message: 'should stay ignored',
                range: range(0, 0, 0, 1),
                source: 'mds-diagnostic-mirror',
              },
            ],
          },
        ],
        expectedCollection: [
          {
            uri: SOURCE_URI,
            diagnostics: [
              {
                message: 'type mismatch',
                range: range(1, 2, 3, 4),
                source: 'tsserver',
              },
            ],
          },
        ],
      },
    ],
  },
  {
    name: 'remap failure suppression',
    steps: [
      {
        name: 'generated diagnostics seed cache before remap loss',
        events: [
          {
            uri: GENERATED_URI,
            diagnostics: [
              {
                message: 'stale generated issue',
                range: range(4, 0, 4, 5),
              },
            ],
          },
        ],
        generatedRemaps: {
          [GENERATED_URI]: SOURCE_URI,
        },
        expectedCollection: [
          {
            uri: SOURCE_URI,
            diagnostics: [
              {
                message: 'stale generated issue',
                range: range(4, 0, 4, 5),
                source: 'generated',
              },
            ],
          },
        ],
      },
      {
        name: 'exact remap loss clears cache instead of guessing onto Markdown',
        events: [
          {
            uri: GENERATED_URI,
            diagnostics: [
              {
                message: 'stale generated issue',
                range: range(4, 0, 4, 5),
              },
            ],
          },
        ],
        generatedRemaps: {
          [GENERATED_URI]: null,
        },
        expectedCollection: [],
      },
    ],
  },
  {
    name: 'embedded stale purge after block move',
    steps: [
      {
        name: 'embedded diagnostics seed cache before block move',
        events: [
          {
            uri: EMBEDDED_URI,
            diagnostics: [
              {
                message: 'unused binding',
                range: range(1, 2, 3, 4),
              },
            ],
          },
        ],
        expectedCollection: [
          {
            uri: SOURCE_URI,
            diagnostics: [
              {
                message: 'unused binding',
                range: range(14, 2, 16, 4),
                source: 'embedded',
              },
            ],
          },
        ],
      },
      {
        name: 'purging invalidated shadow cache drops stale mirror before moved URI repopulates',
        purgeCacheKeys: [EMBEDDED_URI],
        events: [
          {
            uri: EMBEDDED_MOVED_URI,
            diagnostics: [
              {
                message: 'unused binding',
                range: range(1, 2, 3, 4),
              },
            ],
          },
        ],
        expectedCollection: [
          {
            uri: SOURCE_URI,
            diagnostics: [
              {
                message: 'unused binding',
                range: range(24, 2, 26, 4),
                source: 'embedded',
              },
            ],
          },
        ],
      },
    ],
  },
];