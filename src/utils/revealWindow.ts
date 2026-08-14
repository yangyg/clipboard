/**
 * Swap the Vue view first, then show the native window.
 *
 * WebView2 keeps the last compositor frame of a newly-shown window until the
 * next input event. Showing the home list and then flipping to settings leaves
 * the user staring at the list until they click. Flush the new tree while the
 * window is still hidden so WM_PAINT captures SettingsWindow.
 */
export async function revealWindowAfterViewSwap(
  swapView: () => void,
  window: {
    unminimize: () => Promise<void>;
    show: () => Promise<void>;
    setFocus: () => Promise<void>;
  },
  waitForFlush: () => Promise<void>,
): Promise<void> {
  swapView();
  await waitForFlush();
  await window.unminimize();
  await window.show();
  await window.setFocus();
}
