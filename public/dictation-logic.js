(function attachDictationLogic(root, factory) {
  const api = factory();

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  }

  root.DictationLogic = api;
})(typeof globalThis !== 'undefined' ? globalThis : this, () => {
  const DEFAULT_MODEL = 'karanchopda333/whisper:latest';

  function pickDefaultModel(models) {
    if (!Array.isArray(models) || !models.length) return '';
    if (models.includes(DEFAULT_MODEL)) return DEFAULT_MODEL;

    const baseName = DEFAULT_MODEL.split(':')[0];
    const fallbackByBase = models.find((model) => model.startsWith(`${baseName}:`) || model === baseName);
    return fallbackByBase || models[0];
  }

  function withSpeechSupportHint(message, hasSpeechApi) {
    if (hasSpeechApi) return message;
    return `${message} Speech capture unavailable here; paste or type transcript manually.`;
  }

  return {
    DEFAULT_MODEL,
    pickDefaultModel,
    withSpeechSupportHint
  };
});
