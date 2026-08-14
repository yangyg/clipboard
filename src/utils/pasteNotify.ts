/** Toast after `paste_record`: `true` = Ctrl+V was sent, `false` = clipboard only. */
export function toastPasteOutcome(
  injected: boolean,
  mode: "original" | "plain",
  t: (key: string) => string,
  toast: (message: string, kind: "success") => void,
): void {
  if (!injected) {
    toast(t("record.copiedToClipboard"), "success");
    return;
  }
  toast(mode === "plain" ? t("record.pastedPlain") : t("record.pasted"), "success");
}
