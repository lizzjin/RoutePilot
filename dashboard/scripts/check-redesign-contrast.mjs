import { readFile, writeFile } from 'node:fs/promises'

const css = await readFile(new URL('../src/index.css', import.meta.url), 'utf8')
const pairs = [
  ['foreground', 'background'], ['foreground', 'card'], ['body', 'surface-doc'],
  ['muted-foreground', 'background'], ['muted-foreground', 'muted'],
  ['primary-foreground', 'primary'], ['primary-foreground', 'primary-hover'],
  ['primary-foreground', 'primary-pressed'],
  ...['success', 'warning', 'info', 'destructive'].map(name => [`${name}-foreground`, name]),
  ...['success', 'warning', 'info', 'destructive', 'category'].map(name => [`${name}-text`, `${name}-soft`]),
  ['link', 'background'], ['code-foreground', 'code-background'],
  ['input', 'card', 3], ['input', 'background', 3], ['ring', 'background', 3], ['ring', 'card', 3],
]
function luminance(hex) {
  const linear = hex.slice(1).match(/../g).map(value => parseInt(value, 16) / 255)
    .map(value => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4)
  return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
}
const results = []
for (const [theme, selector] of [['light', ':root'], ['dark', '.dark']]) {
  const block = css.split(`${selector} {`)[1].split('}')[0]
  const tokens = Object.fromEntries([...block.matchAll(/--([\w-]+): (#[a-fA-F0-9]{6});/g)].map(match => [match[1], match[2]]))
  for (const [foreground, background, minimum = 4.5] of pairs) {
    const [low, high] = [luminance(tokens[foreground]), luminance(tokens[background])].sort((a, b) => a - b)
    const ratio = (high + 0.05) / (low + 0.05)
    results.push({ theme, foreground, background, foregroundColor: tokens[foreground], backgroundColor: tokens[background], ratio, minimum, passed: ratio >= minimum })
  }
}
await writeFile(new URL('../../docs/assets/routepilot-redesign/contrast-audit.json', import.meta.url), JSON.stringify(results, null, 2) + '\n')
const failed = results.filter(result => !result.passed)
console.log(`${results.length - failed.length}/${results.length} semantic color combinations pass WCAG contrast thresholds`)
if (failed.length) { console.error(failed); process.exitCode = 1 }
