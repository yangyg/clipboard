import { createI18n } from 'vue-i18n'
import zhCN from './zh-CN'
import enUS from './en-US'

const messages = {
  'zh-CN': zhCN,
  'en-US': enUS,
} satisfies Record<string, typeof zhCN>

export const i18n = createI18n({
  legacy: false,
  locale: 'zh-CN',
  fallbackLocale: 'en-US',
  messages,
})

/** Resolve a language setting ('system' | 'zh-CN' | 'en-US') to a concrete locale. */
export function resolveLocale(lang: string): string {
  if (lang === 'system') {
    return navigator.language.startsWith('zh') ? 'zh-CN' : 'en-US'
  }
  return lang
}

/** Switch the active locale at runtime. */
export function setLocale(locale: string) {
  // vue-i18n composition mode: locale is a ref
  ;(i18n.global.locale as unknown as { value: string }).value = locale
  document.documentElement.lang = locale
}
