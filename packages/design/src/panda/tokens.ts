/*
 * Panda tokens are thin aliases of Motet's runtime custom properties.
 * The flat properties (--surface, --accent, --radius-md …) are written by tokens.css for
 * Motet's own appearance and by Motet.applyWorld() on every World scope. Because the
 * aliases are declared on `[data-world]` too (see cssVarRoot), a World's values flow
 * through every recipe without a rebuild.
 */
const v = (name: string) => ({ value: `var(--${name})` })

const statusColors = (n: string) => ({
  DEFAULT: v(n),
  ink: v(`${n}-ink`),
  soft: v(`${n}-soft`),
  on: v(`on-${n}`),
})

export const semanticTokens = {
  colors: {
    canvas: v('canvas'),
    surface: { DEFAULT: v('surface'), quiet: v('surface-quiet'), raised: v('surface-raised'), overlay: v('surface-overlay') },
    ink: { DEFAULT: v('ink'), muted: v('ink-muted') },
    line: { DEFAULT: v('line'), strong: v('line-strong') },
    accent: { DEFAULT: v('accent'), ink: v('accent-ink'), soft: v('accent-soft'), on: v('on-accent'), onSoft: v('on-accent-soft') },
    ornament: { DEFAULT: v('ornament'), soft: v('ornament-soft') },
    focus: v('focus'),
    scrim: v('scrim'),
    selection: v('selection'),
    danger: statusColors('danger'),
    caution: statusColors('caution'),
    positive: statusColors('positive'),
    info: statusColors('info'),
    chrome: {
      bg: v('chrome-bg'), ink: v('chrome-ink'), inkMuted: v('chrome-ink-muted'), line: v('chrome-line'),
      accent: v('chrome-accent'), onAccent: v('chrome-on-accent'), hover: v('chrome-hover'),
    },
  },
  shadows: {
    e1: v('elevation-1'), e2: v('elevation-2'), e3: v('elevation-3'), e4: v('elevation-4'), edge: v('edge-light'),
  },
}

export const tokens = {
  radii: {
    xs: v('radius-xs'), sm: v('radius-sm'), md: v('radius-md'), lg: v('radius-lg'), xl: v('radius-xl'),
    control: v('radius-control'), chrome: { value: '18px' }, full: { value: '9999px' },
  },
  spacing: Object.fromEntries([1, 2, 3, 4, 5, 6, 8, 10, 12, 16].map((n) => [String(n), v(`space-${n}`)])),
  fonts: {
    ui: v('font-ui'), reading: v('font-reading'), display: v('font-display'), name: v('font-name'), mono: v('font-mono'), dice: v('font-dice'),
  },
  durations: {
    spatialFast: v('dur-spatial-fast'), spatial: v('dur-spatial-default'), spatialSlow: v('dur-spatial-slow'),
    effectsFast: v('dur-effects-fast'), effects: v('dur-effects-default'), effectsSlow: v('dur-effects-slow'),
    decor: v('dur-decor'),
  },
  easings: {
    spatialFast: v('ease-spatial-fast'), spatial: v('ease-spatial-default'), spatialSlow: v('ease-spatial-slow'),
    effectsFast: v('ease-effects-fast'), effects: v('ease-effects-default'), effectsSlow: v('ease-effects-slow'),
    decor: v('ease-decor'),
  },
  zIndex: { chrome: { value: '50' }, overlay: { value: '60' }, tooltip: { value: '70' } },
}

/* Transition helpers used by recipes: functional motion is always one of these. */
export const motion = {
  effects: (props: string) =>
    props.split(',').map((p) => `${p.trim()} var(--dur-effects-fast) var(--ease-effects-fast)`).join(', '),
  spatial: (props: string, speed: 'fast' | 'default' | 'slow' = 'fast') =>
    props.split(',').map((p) => `${p.trim()} var(--dur-spatial-${speed}) var(--ease-spatial-${speed})`).join(', '),
}

/* The material stack every World-owned surface uses: finish over grain over the fill. */
export const material = (fill: string) => ({
  backgroundColor: fill,
  backgroundImage: 'var(--finish), var(--grain)',
})
/* Framed surfaces: one solid border colour (--frame-line) and the World's frame profile.
   Profiles may chamfer one or two corners; borders follow the shape. */
export const framed = (fill: string) => ({
  borderWidth: 'var(--border-width)',
  borderStyle: 'solid',
  borderColor: 'var(--frame-line)',
  backgroundColor: fill,
  backgroundImage: 'var(--finish), var(--grain)',
  borderRadius: 'var(--frame-radius)',
  cornerShape: 'var(--frame-shape)',
})

/* Transient surfaces (menus, listboxes, popovers, hover cards): the frosted material.
   Popups live in the top layer through the popover attribute; the engine positions them. */
export const popupSurface = {
  margin: '0', inset: 'auto', padding: '6px', overflow: 'auto',
  border: '1px solid var(--menu-line, var(--line))', borderRadius: 'lg', cornerShape: 'var(--corner-shape)',
  backgroundColor: 'color-mix(in oklab, var(--menu-bg, var(--surface-overlay)) 92%, transparent)', color: 'var(--menu-ink, var(--ink))',
  backdropFilter: 'blur(24px) saturate(1.4)', boxShadow: 'var(--elevation-4), var(--edge-light)',
  fontFamily: 'ui', fontSize: '14px', lineHeight: '20px',
  '[data-reduce-transparency] &': { backgroundColor: 'var(--menu-bg, var(--surface-overlay))', backdropFilter: 'none' },
  '@media (prefers-reduced-transparency: reduce)': { backgroundColor: 'var(--menu-bg, var(--surface-overlay))', backdropFilter: 'none' },
  '&[data-inline]': { position: 'absolute', top: 'calc(100% + 4px)', insetInline: '0 auto', zIndex: '5', width: 'max-content', minWidth: '100%' },
  '&[data-inline][hidden]': { display: 'none' },
}
/* An option row in a listbox: a menu item that can be active (keyboard) and selected. */
export const optionRow = {
  position: 'relative', display: 'flex', alignItems: 'center', gap: '10px', minHeight: '40px', paddingInline: '10px', paddingBlock: '6px',
  borderRadius: 'md', cornerShape: 'var(--corner-shape-control)', color: 'inherit', cursor: 'pointer', userSelect: 'none',
  '& > .m-icon': { color: 'var(--menu-ink-muted, var(--ink-muted))' },
  '&[data-active]': { backgroundColor: 'color-mix(in oklab, currentColor 8%, transparent)' },
  '&[aria-selected=true]': { fontWeight: '600' },
  '&[aria-disabled=true]': { color: 'var(--ink-muted)', cursor: 'not-allowed' },
  '@media (pointer: coarse)': { minHeight: '44px' },
}
/* The bordered box every text-entry control shares; focus rings the whole box. */
export const entryBox = {
  position: 'relative', display: 'flex', alignItems: 'center', width: '100%', minHeight: 'var(--control-h)', minWidth: '0',
  border: '1px solid var(--line-strong)', borderRadius: 'control', cornerShape: 'var(--corner-shape-control)',
  backgroundColor: 'var(--surface-raised)', color: 'var(--ink)', fontFamily: 'ui', fontSize: '14px', lineHeight: '20px',
  transition: motion.effects('border-color, box-shadow'),
  '&:focus-within': { borderColor: 'var(--accent-ink)', outline: '2px solid var(--focus)', outlineOffset: '1px' },
  '&:has([aria-invalid=true])': { borderColor: 'var(--danger-ink)' },
  '&:has(:disabled)': { backgroundColor: 'var(--surface-quiet)', color: 'var(--ink-muted)' },
}
