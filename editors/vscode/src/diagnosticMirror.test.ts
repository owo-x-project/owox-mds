import assert from 'node:assert/strict';
import test from 'node:test';

import {
  applyMirroredDiagnosticEvents,
  createMirroredDiagnosticState,
  mirroredDiagnosticCollectionsEqual,
  parseDiagnosticMirrorUri,
  purgeMirroredDiagnosticCacheKeys,
  shadowDocumentIdentityEqual,
  type DiagnosticMirrorDiagnostic,
  type DiagnosticMirrorRange,
  type DiagnosticMirrorUri,
  type ShadowDocumentIdentity,
} from './diagnosticMirror';
import {
  EMBEDDED_MOVED_URI,
  EMBEDDED_URI,
  diagnosticMirrorRegressionFixtures,
  SOURCE_URI,
  type DiagnosticMirrorRegressionStep,
} from './diagnosticMirrorRegression.fixtures';

function parseFixtureUri(uriText: string): DiagnosticMirrorUri {
  const parsed = parseDiagnosticMirrorUri(uriText);
  assert.ok(parsed, `invalid fixture URI: ${uriText}`);
  return parsed;
}

function sameDiagnosticMirrorUri(
  left: DiagnosticMirrorUri,
  right: DiagnosticMirrorUri
): boolean {
  return (
    left.scheme === right.scheme &&
    left.path === right.path &&
    (left.query || '') === (right.query || '')
  );
}

function createGeneratedRemapResolver(step: DiagnosticMirrorRegressionStep) {
  return async (
    uri: DiagnosticMirrorUri,
    range: DiagnosticMirrorRange
  ) => {
    const event = step.events.find((candidate) =>
      sameDiagnosticMirrorUri(parseFixtureUri(candidate.uri), uri)
    );
    assert.ok(event, `missing fixture event for ${uri.scheme}:${uri.path}`);
    assert.ok(
      step.generatedRemaps &&
      Object.prototype.hasOwnProperty.call(step.generatedRemaps, event.uri),
      `unexpected generated remap request for ${event.uri}`
    );

    const remappedUri = step.generatedRemaps[event.uri];
    if (remappedUri === null) {
      return undefined;
    }

    return {
      uri: remappedUri,
      range,
    };
  };
}

function sameDiagnosticMirrorDiagnostic(
  left: DiagnosticMirrorDiagnostic,
  right: DiagnosticMirrorDiagnostic
): boolean {
  const leftCode = typeof left.code === 'object' ? JSON.stringify(left.code) : left.code;
  const rightCode = typeof right.code === 'object' ? JSON.stringify(right.code) : right.code;

  return left.message === right.message
    && left.source === right.source
    && left.severity === right.severity
    && leftCode === rightCode
    && assertRange(left.range, right.range)
    && assertTags(left.tags, right.tags);
}

function assertRange(
  left: DiagnosticMirrorRange,
  right: DiagnosticMirrorRange
): boolean {
  return left.start.line === right.start.line
    && left.start.character === right.start.character
    && left.end.line === right.end.line
    && left.end.character === right.end.character;
}

function assertTags(
  left: readonly unknown[] | undefined,
  right: readonly unknown[] | undefined
): boolean {
  if (!left && !right) {
    return true;
  }

  if (!left || !right || left.length !== right.length) {
    return false;
  }

  return left.every((tag, index) => tag === right[index]);
}

test('mirrored diagnostic collection equality detects unchanged payloads', () => {
  const left = [
    {
      uri: SOURCE_URI,
      diagnostics: [
        {
          message: 'type mismatch',
          range: {
            start: { line: 1, character: 2 },
            end: { line: 3, character: 4 },
          },
          source: 'tsserver',
        },
      ],
    },
  ];
  const identical = [
    {
      uri: SOURCE_URI,
      diagnostics: [
        {
          message: 'type mismatch',
          range: {
            start: { line: 1, character: 2 },
            end: { line: 3, character: 4 },
          },
          source: 'tsserver',
        },
      ],
    },
  ];
  const changed = [
    {
      uri: SOURCE_URI,
      diagnostics: [
        {
          message: 'type mismatch',
          range: {
            start: { line: 1, character: 2 },
            end: { line: 3, character: 4 },
          },
          source: 'generated',
        },
      ],
    },
  ];

  assert.equal(
    mirroredDiagnosticCollectionsEqual(
      left,
      identical,
      sameDiagnosticMirrorDiagnostic
    ),
    true
  );
  assert.equal(
    mirroredDiagnosticCollectionsEqual(
      left,
      changed,
      sameDiagnosticMirrorDiagnostic
    ),
    false
  );
});

test('shadow document identity changes when embedded startLine moves', () => {
  const original: ShadowDocumentIdentity = {
    content: 'const answer = 42;\n',
    uri: parseFixtureUri(EMBEDDED_URI),
  };
  const moved: ShadowDocumentIdentity = {
    content: 'const answer = 42;\n',
    uri: parseFixtureUri(EMBEDDED_MOVED_URI),
  };

  assert.equal(shadowDocumentIdentityEqual(original, original), true);
  assert.equal(shadowDocumentIdentityEqual(original, moved), false);
});

test('diagnostic mirror regression matrix keeps success, failure, and unmanaged no-op slices distinct', () => {
  const fixtureNames = diagnosticMirrorRegressionFixtures.map((fixture) => fixture.name);

  assert.ok(fixtureNames.includes('generated success'));
  assert.ok(fixtureNames.includes('embedded success'));
  assert.ok(fixtureNames.includes('unmanaged no-op'));
  assert.ok(fixtureNames.includes('remap failure suppression'));
});

for (const fixture of diagnosticMirrorRegressionFixtures) {
  test(`diagnostic mirror regression: ${fixture.name}`, async (t) => {
    const state = createMirroredDiagnosticState<DiagnosticMirrorDiagnostic>();

    for (const step of fixture.steps) {
      await t.test(step.name, async () => {
        if (step.purgeCacheKeys && step.purgeCacheKeys.length > 0) {
          purgeMirroredDiagnosticCacheKeys(state, step.purgeCacheKeys);
        }

        const aggregate = await applyMirroredDiagnosticEvents(
          state,
          step.events.map((event) => ({
            cacheKey: event.uri,
            uri: parseFixtureUri(event.uri),
            diagnostics: event.diagnostics,
          })),
          {
            remapGeneratedRange: createGeneratedRemapResolver(step),
            createDiagnostic: ({ location, diagnostic, fallbackSource }) => ({
              message: diagnostic.message,
              range: location.range,
              source: diagnostic.source ?? fallbackSource,
            }),
          }
        );

        assert.deepEqual(aggregate, step.expectedCollection);
      });
    }
  });
}