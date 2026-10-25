import { defineRecipe, defineSlotRecipe } from '@pandacss/dev'
import { motion } from '../tokens'

/* Feedback and data display. Status always pairs an icon with words; loading never shimmers,
   and indeterminate motion stops under reduced motion while the words stay. */

const tones = (k: string) => ({ root: { '--tone-soft': `var(--${k}-soft)`, '--tone-ink': `var(--${k}-ink)` } })

export const alert = defineSlotRecipe({
  className: 'm-alert',
  description: 'A message about this page or form that stays until its cause is gone.',
  slots: ['root', 'icon', 'content', 'title', 'description', 'actions', 'close'],
  base: {
    root: {
      '--tone-soft': 'var(--info-soft)', '--tone-ink': 'var(--info-ink)',
      display: 'grid', gridTemplateColumns: 'auto minmax(0, 1fr) auto', columnGap: '12px', alignItems: 'start', paddingInline: '14px 8px', paddingBlock: '12px',
      borderRadius: 'md', cornerShape: 'var(--corner-shape-control)', backgroundColor: 'var(--tone-soft)', color: 'var(--ink)', fontFamily: 'ui',
    },
    icon: { color: 'var(--tone-ink)', marginTop: '0' },
    content: { display: 'grid', gap: '2px', minWidth: '0', paddingTop: '0', paddingInlineEnd: '6px' },
    title: { margin: '0', fontSize: '14px', lineHeight: '20px', fontWeight: '600' },
    description: { fontSize: '13.5px', lineHeight: '20px', '& p': { margin: '0' } },
    actions: { display: 'flex', flexWrap: 'wrap', gap: '8px', marginTop: '8px', '& .m-button': { minHeight: '32px' } },
    close: { marginBlock: '-6px', '& .m-icon-button': { width: '32px', height: '32px', color: 'var(--ink-muted)' } },
  } as any,
  variants: {
    tone: { info: tones('info'), caution: tones('caution'), danger: tones('danger'), positive: tones('positive'), neutral: { root: { '--tone-soft': 'var(--surface-quiet)', '--tone-ink': 'var(--ink-muted)' } } },
    variant: { inline: {}, banner: { root: { borderRadius: '0', cornerShape: 'round', paddingInline: '16px 8px' } } },
  } as any,
  staticCss: [{ tone: ['*'], variant: ['*'] }],
})

export const progress = defineSlotRecipe({
  className: 'm-progress',
  description: 'How far a known task has got, or that an unknown one is running.',
  slots: ['root', 'header', 'label', 'value', 'track', 'fill'],
  base: {
    root: { display: 'grid', gap: '6px', fontFamily: 'ui', minWidth: '0' },
    header: { display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', gap: '12px', fontSize: '13px', lineHeight: '18px' },
    label: { fontWeight: '600', color: 'var(--ink)' },
    value: { color: 'var(--ink-muted)', fontVariantNumeric: 'tabular-nums' },
    track: { position: 'relative', height: '6px', overflow: 'hidden', borderRadius: '3px', backgroundColor: 'var(--surface-quiet)', boxShadow: 'inset 0 0 0 1px var(--line)' },
    fill: {
      height: '100%', width: 'var(--value, 0%)', borderRadius: '3px', backgroundColor: 'var(--accent)', transition: motion.spatial('width', 'slow'),
      '[data-state=indeterminate] &': { width: '35%', animation: 'm-indeterminate 1.6s var(--ease-effects-default) infinite' },
      '[data-tone=danger] &': { backgroundColor: 'var(--danger)' },
      '@media (prefers-reduced-motion: reduce)': { '[data-state=indeterminate] &': { animation: 'none', width: '100%', opacity: '0.45' } },
    },
  } as any,
})

export const ring = defineSlotRecipe({
  className: 'm-ring',
  description: 'Circular progress, and the spinner when nothing better is known.',
  slots: ['root', 'svg', 'track', 'arc', 'value'],
  base: {
    root: { position: 'relative', display: 'inline-grid', placeItems: 'center', width: 'var(--ring-size, 40px)', height: 'var(--ring-size, 40px)', flex: 'none', color: 'var(--accent)' },
    svg: {
      width: '100%', height: '100%', rotate: '-90deg',
      '[data-state=indeterminate] &': { animation: 'm-spin 0.9s linear infinite' },
      '@media (prefers-reduced-motion: reduce)': { '[data-state=indeterminate] &': { animation: 'none' } },
    },
    track: { fill: 'none', stroke: 'var(--surface-quiet)', strokeWidth: 'var(--ring-stroke, 4)' },
    arc: { fill: 'none', stroke: 'currentColor', strokeWidth: 'var(--ring-stroke, 4)', strokeLinecap: 'round', strokeDasharray: 'var(--value, 25) 100', transition: 'stroke-dasharray var(--dur-spatial-slow) var(--ease-spatial-slow)' },
    value: { position: 'absolute', fontFamily: 'ui', fontSize: 'calc(var(--ring-size, 40px) * 0.28)', fontWeight: '600', color: 'var(--ink)', fontVariantNumeric: 'tabular-nums' },
  } as any,
})

export const skeleton = defineRecipe({
  className: 'm-skeleton',
  description: 'A static placeholder in the shape of content that is loading. It never shimmers.',
  base: { display: 'block', backgroundColor: 'color-mix(in oklab, var(--ink) 9%, transparent)', borderRadius: 'sm', cornerShape: 'var(--corner-shape-control)' },
  variants: {
    shape: {
      line: { height: '0.8em', marginBlock: '0.35em', borderRadius: '4px' },
      block: {},
      circle: { borderRadius: '50%', cornerShape: 'round' },
    },
  } as any,
  staticCss: [{ shape: ['*'] }],
})

export const datalist = defineSlotRecipe({
  className: 'm-datalist',
  description: 'Labelled facts: a character sheet, scene details, record metadata.',
  slots: ['root', 'item', 'label', 'value'],
  base: {
    root: { display: 'grid', margin: '0', fontFamily: 'ui' },
    item: { display: 'grid', gridTemplateColumns: 'var(--label-w, minmax(96px, 38%)) minmax(0, 1fr)', columnGap: '16px', alignItems: 'baseline', paddingBlock: '6px' },
    label: { fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)' },
    value: { margin: '0', fontSize: '14px', lineHeight: '20px', color: 'var(--ink)', fontVariantNumeric: 'var(--numerals, normal)', overflowWrap: 'anywhere' },
  } as any,
  variants: {
    variant: {
      plain: {},
      ruled: { item: { paddingBlock: '9px', boxShadow: 'inset 0 -1px 0 var(--line)', '&:last-child': { boxShadow: 'none' } } },
      stacked: { root: { gridTemplateColumns: 'repeat(auto-fill, minmax(120px, 1fr))', gap: '12px 20px' }, item: { gridTemplateColumns: 'minmax(0, 1fr)', rowGap: '2px', paddingBlock: '0' } },
    },
  } as any,
  staticCss: [{ variant: ['*'] }],
})
