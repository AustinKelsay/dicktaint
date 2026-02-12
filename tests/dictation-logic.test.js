const { describe, it, expect } = require('bun:test');

const {
  DEFAULT_MODEL,
  pickDefaultModel,
  withSpeechSupportHint
} = require('../public/dictation-logic.js');

describe('dictation logic', () => {
  it('uses configured default model when available', () => {
    const models = ['foo:latest', DEFAULT_MODEL, 'bar:latest'];
    expect(pickDefaultModel(models)).toBe(DEFAULT_MODEL);
  });

  it('falls back to same base name when default tag is missing', () => {
    const models = ['llama3.2:1b', 'foo:latest'];
    expect(pickDefaultModel(models)).toBe('llama3.2:1b');
  });

  it('falls back to first model for unrelated list', () => {
    const models = ['alpha:1', 'beta:2'];
    expect(pickDefaultModel(models)).toBe('alpha:1');
  });

  it('returns empty string when models are missing', () => {
    expect(pickDefaultModel([])).toBe('');
    expect(pickDefaultModel(null)).toBe('');
  });

  it('adds unsupported speech hint only when speech api is unavailable', () => {
    const message = 'Connected.';
    expect(withSpeechSupportHint(message, true)).toBe(message);
    expect(withSpeechSupportHint(message, false)).toContain('Speech capture unavailable here');
  });
});
