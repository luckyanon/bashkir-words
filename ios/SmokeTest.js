(async () => {
  const bridge = window.webkit.messageHandlers.bashkir;
  const waitFor = async predicate => {
    for (let i = 0; i < 200; i++) {
      if (predicate()) return;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    throw Error('Timed out: ' + document.body.innerText.slice(0, 500));
  };
  try {
    await waitFor(() => document.querySelector('.menu-screen'));
    await document.fonts.ready;
    if (!document.fonts.check('16px Game')) throw Error('Bundled font did not load');
    const preferences = window.BashkirNativePlugins.Preferences;
    const original = (await preferences.get({ key: 'bashkir-words:save' })).value;
    const expected = sessionStorage.getItem('ios-smoke-save');
    if (expected) {
      if (original !== expected) throw Error('Progress did not survive reload');
      document.querySelector('[data-action="continue"]').click();
      await waitFor(() => document.querySelector('.crossword .cell'));
      await bridge.postMessage({ operation: 'smokeResult', result: { status: 'passed', checks: ['offline dictionary', 'fonts', 'classic board', 'hint', 'native save', 'webview reload'] } });
      return;
    }
    document.querySelector('[data-action="continue"]').click();
    await waitFor(() => document.querySelector('.crossword .cell'));
    document.querySelector('[data-action="hint"]').click();
    await waitFor(() => document.querySelector('.cell.revealed'));
    await new Promise(resolve => setTimeout(resolve, 700));
    const save = (await preferences.get({ key: 'bashkir-words:save' })).value;
    if (!save || JSON.parse(save).coins !== 70) throw Error('Hint progress did not save');
    sessionStorage.setItem('ios-smoke-save', save);
    location.reload();
  } catch (error) {
    await bridge.postMessage({ operation: 'smokeResult', result: { status: 'failed', error: String(error) } });
  }
})();
