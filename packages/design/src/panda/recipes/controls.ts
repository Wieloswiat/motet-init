import { defineRecipe, defineSlotRecipe } from '@pandacss/dev'
import { motion } from '../tokens'

/* A state layer tints whatever the control is painted with, as Material does:
   hover 8%, pressed 14% of the content colour. It never replaces the fill. */
export const stateLayer = {
  backgroundImage: 'linear-gradient(var(--state, transparent), var(--state, transparent))',
  '@media (hover: hover)': { '&:hover:not(:disabled)': { '--state': 'color-mix(in oklab, currentColor 8%, transparent)' } },
  '&:active:not(:disabled)': { '--state': 'color-mix(in oklab, currentColor 14%, transparent)' },
} as const

/* A small control keeps a 44px hit area on coarse pointers without growing visually. */
export const coarseHitArea = {
  '@media (pointer: coarse)': {
    '&::before': { content: '""', position: 'absolute', inset: '-6px', borderRadius: 'inherit' },
  },
} as const

export const button = defineRecipe({
  className: 'm-button',
  description: 'Actions. One filled button per view; tonal for the rest; text inside dense content.',
  base: {
    '--btn-fill': 'var(--accent)', '--btn-on': 'var(--on-accent)', '--btn-soft': 'var(--accent-soft)',
    '--btn-on-soft': 'var(--on-accent-soft)', '--btn-ink': 'var(--accent-ink)',
    position: 'relative', display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: '8px',
    minHeight: 'var(--control-h)', paddingInline: '16px', paddingBlock: '6px',
    borderRadius: 'control', cornerShape: 'var(--corner-shape-control)',
    border: '1px solid transparent', fontFamily: 'ui', fontSize: '14px', lineHeight: '20px', fontWeight: '600',
    letterSpacing: '0.005em', textAlign: 'center', textWrap: 'balance', textDecoration: 'none',
    cursor: 'pointer', userSelect: 'none', WebkitTapHighlightColor: 'transparent',
    transition: `${motion.effects('background-color, color, border-color, box-shadow')}, ${motion.spatial('border-radius')}`,
    '& .m-icon': { fontSize: '20px', marginInline: '-2px' },
    '&:disabled': { cursor: 'not-allowed', backgroundColor: 'var(--surface-quiet) !important', color: 'var(--ink-muted) !important', borderColor: 'transparent !important', boxShadow: 'none !important' },
    ...stateLayer,
    '&:active:not(:disabled)': { ...stateLayer['&:active:not(:disabled)'], borderRadius: 'calc(var(--radius-control) * 0.55)' },
    ...coarseHitArea,
  } as any,
  variants: {
    variant: {
      filled: { backgroundColor: 'var(--btn-fill)', color: 'var(--btn-on)', boxShadow: 'var(--edge-light), var(--elevation-1)' },
      tonal: { backgroundColor: 'var(--btn-soft)', color: 'var(--btn-on-soft)' },
      outline: { backgroundColor: 'transparent', color: 'var(--btn-ink)', borderColor: 'var(--line-strong)' },
      text: { backgroundColor: 'transparent', color: 'var(--btn-ink)', paddingInline: '12px' },
    },
    tone: {
      accent: {},
      danger: { '--btn-fill': 'var(--danger)', '--btn-on': 'var(--on-danger)', '--btn-soft': 'var(--danger-soft)', '--btn-on-soft': 'var(--danger-ink)', '--btn-ink': 'var(--danger-ink)' },
      neutral: { '--btn-fill': 'var(--ink)', '--btn-on': 'var(--surface)', '--btn-soft': 'var(--surface-quiet)', '--btn-on-soft': 'var(--ink)', '--btn-ink': 'var(--ink)' },
    },
    size: {
      sm: { minHeight: '32px', paddingInline: '12px', paddingBlock: '4px', fontSize: '13px', gap: '6px', '& .m-icon': { fontSize: '18px' } },
      md: {},
    },
  } as any,
  staticCss: [{ variant: ['*'], tone: ['*'], size: ['*'] }],
})

export const iconButton = defineRecipe({
  className: 'm-icon-button',
  description: 'An icon action. Always has an accessible name, shown as a tooltip.',
  base: {
    position: 'relative', display: 'inline-grid', placeItems: 'center', flex: 'none',
    width: 'var(--control-h)', height: 'var(--control-h)', padding: '0', border: '0',
    borderRadius: 'control', cornerShape: 'var(--corner-shape-control)',
    backgroundColor: 'transparent', color: 'var(--ink-muted)', cursor: 'pointer',
    transition: `${motion.effects('background-color, color')}, ${motion.spatial('border-radius')}`,
    '&:active': { borderRadius: 'calc(var(--radius-control) * 0.55)' },
    '&[aria-pressed=true], &[aria-expanded=true]': { color: 'var(--on-accent-soft)', backgroundColor: 'var(--accent-soft)' },
    '& .m-icon-button__count': {
      position: 'absolute', top: '1px', insetInlineEnd: '1px', minWidth: '18px', height: '18px', paddingInline: '5px',
      borderRadius: '9px', backgroundColor: 'var(--accent)', color: 'var(--on-accent)', boxShadow: '0 0 0 2px var(--count-ring, var(--surface))',
      font: '600 11px/18px var(--font-ui)', fontVariantNumeric: 'tabular-nums', textAlign: 'center', pointerEvents: 'none',
    },
    ...stateLayer,
    '@media (hover: hover)': { ...stateLayer['@media (hover: hover)'], '&:hover': { color: 'var(--ink)' } },
    ...coarseHitArea,
  } as any,
  variants: {
    variant: {
      standard: {},
      tonal: { backgroundColor: 'var(--surface-quiet)', color: 'var(--ink)' },
      filled: { backgroundColor: 'var(--accent)', color: 'var(--on-accent)', '@media (hover: hover)': { '&:hover': { color: 'var(--on-accent)' } } },
    },
    size: {
      sm: { width: '32px', height: '32px', '& .m-icon': { fontSize: '18px' } },
      md: {},
    },
  } as any,
  staticCss: [{ variant: ['*'], size: ['*'] }],
})

export const chip = defineRecipe({
  className: 'm-chip',
  description: 'Filters, content notes and short facts. Never a status dot.',
  base: {
    position: 'relative', display: 'inline-flex', alignItems: 'center', gap: '6px', minHeight: '32px', paddingInline: '12px',
    borderRadius: 'sm', cornerShape: 'var(--corner-shape-control)', border: '1px solid var(--line-strong)',
    fontFamily: 'ui', fontSize: '13px', lineHeight: '18px', fontWeight: '500', color: 'var(--ink)', backgroundColor: 'transparent',
    whiteSpace: 'nowrap', transition: motion.effects('background-color, color, border-color'),
    '& .m-icon': { fontSize: '18px', marginInlineStart: '-4px' },
  } as any,
  variants: {
    kind: {
      filter: {
        cursor: 'pointer', ...stateLayer, ...coarseHitArea,
        '&[aria-pressed=true]': { backgroundColor: 'var(--accent-soft)', color: 'var(--on-accent-soft)', borderColor: 'transparent' },
      },
      note: { borderColor: 'transparent', backgroundColor: 'var(--surface-quiet)', color: 'var(--ink-muted)', minHeight: '28px', '& .m-icon': { color: 'var(--info-ink)' } },
      fact: { borderColor: 'transparent', backgroundColor: 'var(--surface-quiet)', minHeight: '28px' },
    },
    tone: {
      caution: { backgroundColor: 'var(--caution-soft)', color: 'var(--caution-ink)', borderColor: 'transparent', '& .m-icon': { color: 'inherit' } },
      danger: { backgroundColor: 'var(--danger-soft)', color: 'var(--danger-ink)', borderColor: 'transparent', '& .m-icon': { color: 'inherit' } },
      positive: { backgroundColor: 'var(--positive-soft)', color: 'var(--positive-ink)', borderColor: 'transparent', '& .m-icon': { color: 'inherit' } },
    },
  } as any,
  staticCss: [{ kind: ['*'], tone: ['*'] }],
})

/* Account avatars are always round (platform identity); character portraits and World
   marks take the World's corner shape. The two must never be confused. */
export const avatar = defineRecipe({
  className: 'm-avatar',
  base: {
    position: 'relative', display: 'inline-grid', placeItems: 'center', flex: 'none', overflow: 'hidden',
    width: 'var(--avatar-size)', height: 'var(--avatar-size)',
    backgroundColor: 'oklch(0.86 0.06 var(--avatar-hue, 220))', color: 'oklch(0.3 0.06 var(--avatar-hue, 220))',
    fontFamily: 'ui', fontWeight: '600', fontSize: 'calc(var(--avatar-size) * 0.4)', lineHeight: '1',
    '[data-theme=dark] &, [data-theme=contrast-dark] &, [data-mode=dark] &': { backgroundColor: 'oklch(0.42 0.07 var(--avatar-hue, 220))', color: 'oklch(0.95 0.02 var(--avatar-hue, 220))' },
    '[data-mode=light] &': { backgroundColor: 'oklch(0.86 0.06 var(--avatar-hue, 220))', color: 'oklch(0.3 0.06 var(--avatar-hue, 220))' },
    '& img': { width: '100%', height: '100%', objectFit: 'cover', display: 'block' },
  } as any,
  variants: {
    kind: {
      account: { borderRadius: '50%', cornerShape: 'round', boxShadow: 'inset 0 0 0 1px color-mix(in oklab, var(--ink) 12%, transparent)' },
      character: { borderRadius: 'calc(var(--avatar-size) * 0.16 * var(--radius-md) / 12px)', cornerShape: 'var(--corner-shape)', boxShadow: 'inset 0 0 0 1px color-mix(in oklab, var(--ink) 14%, transparent)' },
      world: { borderRadius: 'calc(var(--avatar-size) * 0.22)', cornerShape: 'squircle' },
    },
    size: {
      xs: { '--avatar-size': '22px' }, sm: { '--avatar-size': '28px' }, md: { '--avatar-size': '40px' },
      lg: { '--avatar-size': '56px' }, xl: { '--avatar-size': '88px' },
    },
  } as any,
  staticCss: [{ kind: ['*'], size: ['*'] }],
})

export const segmented = defineSlotRecipe({
  className: 'm-segmented',
  description: 'Two to four mutually exclusive views, e.g. In character / Out of character.',
  slots: ['root', 'item'],
  base: {
    root: {
      display: 'inline-flex', padding: '3px', gap: '2px', backgroundColor: 'var(--surface-quiet)',
      borderRadius: 'calc(var(--radius-control) + 3px)', cornerShape: 'var(--corner-shape-control)',
      boxShadow: 'inset 0 0 0 1px var(--line)',
    },
    item: {
      position: 'relative', display: 'inline-flex', alignItems: 'center', gap: '6px', minHeight: 'calc(var(--control-h) - 6px)',
      paddingInline: '14px', border: '0', borderRadius: 'control', cornerShape: 'var(--corner-shape-control)',
      backgroundColor: 'transparent', color: 'var(--ink-muted)', fontFamily: 'ui', fontSize: '13px', fontWeight: '600', lineHeight: '18px',
      cursor: 'pointer', transition: `${motion.effects('background-color, color, box-shadow')}`,
      '& .m-icon': { fontSize: '18px' },
      '@media (hover: hover)': { '&:hover': { color: 'var(--ink)' } },
      '&[aria-checked=true]': { backgroundColor: 'var(--surface-raised)', color: 'var(--ink)', boxShadow: 'var(--elevation-1), var(--edge-light)' },
      '@media (pointer: coarse)': { minHeight: '38px' },
    },
  } as any,
})

export const switchRecipe = defineSlotRecipe({
  className: 'm-switch',
  slots: ['root', 'thumb'],
  base: {
    root: {
      position: 'relative', display: 'inline-flex', alignItems: 'center', flex: 'none', width: '48px', height: '28px', padding: '0 4px',
      border: '2px solid var(--line-strong)', borderRadius: '9999px', backgroundColor: 'var(--surface-quiet)', cursor: 'pointer',
      transition: motion.effects('background-color, border-color'),
      '&[aria-checked=true]': { backgroundColor: 'var(--accent)', borderColor: 'var(--accent)' },
      ...coarseHitArea,
    },
    thumb: {
      width: '16px', height: '16px', borderRadius: '9999px', backgroundColor: 'var(--ink-muted)',
      transition: `${motion.spatial('translate, width, height')}, ${motion.effects('background-color')}`,
      '[aria-checked=true] > &': { translate: '20px 0', width: '18px', height: '18px', backgroundColor: 'var(--on-accent)' },
      '[aria-checked=false]:active > &': { width: '20px' },
    },
  } as any,
})

export const field = defineSlotRecipe({
  className: 'm-field',
  slots: ['root', 'label', 'control', 'help'],
  base: {
    root: { display: 'grid', gap: '6px', minWidth: '0' },
    label: { fontFamily: 'ui', fontSize: '13px', fontWeight: '600', lineHeight: '18px', color: 'var(--ink)' },
    control: {
      width: '100%', minHeight: 'var(--control-h)', paddingInline: '12px', paddingBlock: '8px',
      border: '1px solid var(--line-strong)', borderRadius: 'control', cornerShape: 'var(--corner-shape-control)',
      backgroundColor: 'var(--surface-raised)', color: 'var(--ink)', fontFamily: 'ui', fontSize: '14px', lineHeight: '20px',
      transition: motion.effects('border-color, box-shadow'),
      '&::placeholder': { color: 'var(--ink-muted)' },
      '&:focus': { borderColor: 'var(--accent-ink)', outline: '2px solid var(--focus)', outlineOffset: '1px' },
      '&[aria-invalid=true]': { borderColor: 'var(--danger-ink)' },
      'textarea&': { minHeight: '96px', resize: 'vertical', fontFamily: 'reading', fontSize: '16px', lineHeight: '1.55' },
    },
    help: {
      display: 'flex', gap: '6px', alignItems: 'flex-start', fontFamily: 'ui', fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)',
      '& .m-icon': { fontSize: '18px' },
      '&[data-tone=danger]': { color: 'var(--danger-ink)' },
      '&[data-tone=caution]': { color: 'var(--caution-ink)' },
      '&[hidden]': { display: 'none' },
    },
  } as any,
})

export const tabs = defineSlotRecipe({
  className: 'm-tabs',
  slots: ['root', 'tab'],
  base: {
    root: { display: 'flex', gap: '4px', overflowX: 'auto', scrollbarWidth: 'none', boxShadow: 'inset 0 -1px 0 var(--line)' },
    tab: {
      position: 'relative', display: 'inline-flex', alignItems: 'center', gap: '8px', minHeight: '44px', paddingInline: '12px',
      color: 'var(--ink-muted)', textDecoration: 'none', fontFamily: 'ui', fontSize: '14px', fontWeight: '600', whiteSpace: 'nowrap',
      borderRadius: 'var(--radius-sm) var(--radius-sm) 0 0', transition: motion.effects('color'),
      '& .m-icon': { fontSize: '20px' },
      '@media (hover: hover)': { '&:hover': { color: 'var(--ink)' } },
      '&::after': {
        content: '""', position: 'absolute', insetInline: '12px', bottom: '0', height: '3px', borderRadius: '3px 3px 0 0',
        backgroundColor: 'var(--accent)', scale: '0 1', transition: motion.spatial('scale', 'default'),
      },
      '&[aria-current=page], &[aria-selected=true]': { color: 'var(--ink)', '&::after': { scale: '1 1' } },
    },
  } as any,
})

export const tooltip = defineRecipe({
  className: 'm-tooltip',
  base: {
    position: 'absolute', zIndex: 'tooltip', translate: '-50% 4px', opacity: '0', pointerEvents: 'none', width: 'max-content', maxWidth: '240px',
    paddingInline: '8px', paddingBlock: '4px', borderRadius: '8px', backgroundColor: 'var(--ink)', color: 'var(--surface)',
    fontFamily: 'ui', fontSize: '12.5px', lineHeight: '17px', fontWeight: '500', boxShadow: 'var(--elevation-2)',
    transition: `${motion.effects('opacity')}, translate var(--dur-spatial-fast) var(--ease-spatial-fast)`,
    '&[data-open]': { opacity: '1', translate: '-50% 0' },
  } as any,
})

export const menu = defineSlotRecipe({
  className: 'm-menu',
  description: 'Transient list of actions or destinations. Uses the frosted material.',
  slots: ['root', 'label', 'item', 'separator', 'hint'],
  base: {
    root: {
      minWidth: '240px', maxWidth: 'min(360px, calc(100vw - 24px))', padding: '6px', margin: '0',
      border: '1px solid var(--menu-line, var(--line))', borderRadius: 'lg', cornerShape: 'var(--corner-shape)',
      backgroundColor: 'color-mix(in oklab, var(--menu-bg, var(--surface-overlay)) 92%, transparent)', color: 'var(--menu-ink, var(--ink))',
      backdropFilter: 'blur(24px) saturate(1.4)', boxShadow: 'var(--elevation-4), var(--edge-light)',
      fontFamily: 'ui', fontSize: '14px', lineHeight: '20px',
      '[data-reduce-transparency] &': { backgroundColor: 'var(--menu-bg, var(--surface-overlay))', backdropFilter: 'none' },
      '@media (prefers-reduced-transparency: reduce)': { backgroundColor: 'var(--menu-bg, var(--surface-overlay))', backdropFilter: 'none' },
    },
    label: { paddingInline: '10px', paddingBlock: '8px 4px', fontSize: '12.5px', fontWeight: '600', color: 'var(--menu-ink-muted, var(--ink-muted))' },
    item: {
      position: 'relative', display: 'flex', alignItems: 'center', gap: '12px', width: '100%', minHeight: '40px', paddingInline: '10px',
      border: '0', borderRadius: 'md', cornerShape: 'var(--corner-shape-control)', backgroundColor: 'transparent', color: 'inherit',
      font: 'inherit', textAlign: 'start', textDecoration: 'none', cursor: 'pointer',
      '& > .m-icon': { color: 'var(--menu-ink-muted, var(--ink-muted))' },
      '@media (hover: hover)': { '&:hover': { backgroundColor: 'color-mix(in oklab, currentColor 8%, transparent)' } },
      '&[aria-checked=true], &[aria-current=true]': { backgroundColor: 'color-mix(in oklab, var(--menu-accent, var(--accent)) 16%, transparent)', '& > .m-icon': { color: 'inherit' } },
      '&[data-tone=danger]': { color: 'var(--danger-ink)', '& > .m-icon': { color: 'inherit' } },
      '@media (pointer: coarse)': { minHeight: '44px' },
    },
    separator: { height: '1px', marginBlock: '6px', marginInline: '4px', backgroundColor: 'var(--menu-line, var(--line))', border: '0' },
    hint: { marginInlineStart: 'auto', color: 'var(--menu-ink-muted, var(--ink-muted))', fontSize: '12.5px', fontVariantNumeric: 'tabular-nums' },
  } as any,
})

export const dialog = defineSlotRecipe({
  className: 'm-dialog',
  slots: ['scrim', 'root', 'header', 'title', 'body', 'actions'],
  base: {
    /* for a dialog drawn without <dialog>: the dimmed layer that centres it */
    scrim: {
      position: 'fixed', inset: '0', zIndex: '80', display: 'grid', placeItems: 'center', padding: '16px', backgroundColor: 'var(--scrim)',
      animation: 'm-fade-in var(--dur-effects-default) var(--ease-effects-default)',
    },
    /* never taller than the window: the content scrolls and the actions stay in reach */
    root: {
      width: 'min(480px, calc(100vw - 32px))', maxHeight: 'calc(100dvh - 32px)', overflowY: 'auto', overscrollBehavior: 'contain',
      padding: '24px', border: '1px solid var(--line)', borderRadius: 'xl', cornerShape: 'var(--corner-shape)',
      backgroundColor: 'var(--surface-raised)', color: 'var(--ink)', boxShadow: 'var(--elevation-4), var(--edge-light)',
      animation: 'm-dialog-in var(--dur-spatial-default) var(--ease-spatial-default)',
      '&::backdrop': { backgroundColor: 'var(--scrim)', animation: 'm-fade-in var(--dur-effects-default) var(--ease-effects-default)' },
    },
    header: { display: 'flex', alignItems: 'flex-start', gap: '12px', marginBottom: '8px', '& > .m-icon': { fontSize: '24px', marginTop: '2px' } },
    title: { margin: '0', fontFamily: 'ui', fontSize: '20px', lineHeight: '28px', fontWeight: '600' },
    body: { fontFamily: 'ui', fontSize: '14px', lineHeight: '21px', color: 'var(--ink-muted)', '& p': { margin: '0 0 8px' }, '& strong': { color: 'var(--ink)' } },
    actions: {
      position: 'sticky', bottom: '-24px', zIndex: '1', display: 'flex', flexWrap: 'wrap', justifyContent: 'flex-end', gap: '8px',
      marginTop: '8px', marginInline: '-24px', marginBottom: '-24px', paddingInline: '24px', paddingBlock: '12px 24px',
      backgroundColor: 'var(--surface-raised)', borderRadius: '0 0 var(--radius-xl) var(--radius-xl)',
    },
  } as any,
})

export const flag = defineRecipe({
  className: 'm-flag',
  description: 'A labelled state on a row: Your turn, Draft, Done. Icon plus word, never colour alone.',
  base: {
    display: 'inline-flex', alignItems: 'center', gap: '4px', minHeight: '24px', paddingInline: '8px 10px', borderRadius: 'sm',
    cornerShape: 'var(--corner-shape-control)', fontFamily: 'ui', fontSize: '12.5px', lineHeight: '16px', fontWeight: '600', whiteSpace: 'nowrap',
    '& .m-icon': { fontSize: '16px' },
  } as any,
  variants: {
    kind: {
      turn: { backgroundColor: 'var(--accent-soft)', color: 'var(--on-accent-soft)' },
      draft: { backgroundColor: 'var(--surface-quiet)', color: 'var(--ink)', boxShadow: 'inset 0 0 0 1px var(--line)' },
      done: { backgroundColor: 'transparent', color: 'var(--positive-ink)' },
      caution: { backgroundColor: 'var(--caution-soft)', color: 'var(--caution-ink)' },
      danger: { backgroundColor: 'var(--danger-soft)', color: 'var(--danger-ink)' },
    },
  } as any,
  staticCss: [{ kind: ['*'] }],
})
