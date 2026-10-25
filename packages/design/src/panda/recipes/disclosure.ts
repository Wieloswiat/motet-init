import { defineSlotRecipe } from '@pandacss/dev'
import { framed, motion } from '../tokens'
import { coarseHitArea, stateLayer } from './controls'

/* Disclosure and navigation: accordions and collapsibles are native <details> or a button
   and region, so find-in-page reveals hidden text; trees, pagination, breadcrumbs and steps
   follow the ARIA patterns with roving focus where the pattern asks for it. */

const openSpring = `block-size var(--dur-spatial-default) var(--ease-spatial-default), content-visibility var(--dur-spatial-default) allow-discrete`

export const accordion = defineSlotRecipe({
  className: 'm-accordion',
  description: 'Sections a reader opens one or several at a time: rules, lore chapters, settings.',
  slots: ['root', 'item', 'trigger', 'title', 'meta', 'indicator', 'content'],
  base: {
    root: { display: 'grid', fontFamily: 'ui', color: 'var(--ink)' },
    item: {
      interpolateSize: 'allow-keywords', boxShadow: 'inset 0 -1px 0 var(--line)',
      '&::details-content': { blockSize: '0', overflowY: 'clip', transition: openSpring },
      '&[open]::details-content': { blockSize: 'auto' },
    },
    trigger: {
      position: 'relative', display: 'flex', alignItems: 'center', gap: '12px', minHeight: '52px', paddingInline: '4px', paddingBlock: '10px',
      listStyle: 'none', cursor: 'pointer', borderRadius: 'md', cornerShape: 'var(--corner-shape-control)', WebkitTapHighlightColor: 'transparent',
      '&::-webkit-details-marker': { display: 'none' },
      '&::marker': { content: '""' },
      '& > .m-icon:first-child': { color: 'var(--ink-muted)' },
      '&:focus-visible': { outline: '2px solid var(--focus)', outlineOffset: '-2px' },
      '@media (hover: hover)': { '&:hover .m-accordion__title': { color: 'var(--accent-ink)' } },
    },
    title: { flex: '1', minWidth: '0', fontSize: '15px', lineHeight: '22px', fontWeight: '600', transition: motion.effects('color') },
    meta: { fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)', fontVariantNumeric: 'tabular-nums', whiteSpace: 'nowrap' },
    indicator: { color: 'var(--ink-muted)', transition: motion.spatial('rotate'), '[open] > summary > &': { rotate: '180deg' } },
    content: { paddingInline: '4px', paddingBottom: '16px', fontSize: '14px', lineHeight: '21px' },
  } as any,
  variants: {
    variant: {
      flush: {},
      framed: {
        root: { ...framed('var(--surface-raised)'), overflow: 'clip', boxShadow: 'var(--elevation-1), var(--edge-light)' },
        item: { '&:last-child': { boxShadow: 'none' } },
        trigger: { paddingInline: '16px', borderRadius: '0' },
        content: { paddingInline: '16px' },
      },
    },
  } as any,
  staticCss: [{ variant: ['*'] }],
})

export const collapsible = defineSlotRecipe({
  className: 'm-collapsible',
  description: 'One region a reader shows or hides: more detail, or a spoiler inside a post.',
  slots: ['root', 'trigger', 'label', 'topic', 'action', 'indicator', 'content'],
  base: {
    root: { fontFamily: 'ui' },
    trigger: {
      position: 'relative', display: 'inline-flex', alignItems: 'center', gap: '4px', minHeight: '36px', paddingInline: '8px', marginInline: '-8px',
      border: '0', borderRadius: 'control', cornerShape: 'var(--corner-shape-control)', backgroundColor: 'transparent', color: 'var(--accent-ink)',
      font: '600 14px/20px var(--font-ui)', cursor: 'pointer', ...stateLayer, ...coarseHitArea,
    },
    label: {},
    topic: {},
    action: {},
    indicator: { transition: motion.spatial('rotate'), '[aria-expanded=true] > &': { rotate: '180deg' } },
    content: { paddingTop: '8px', '&:not([hidden])': { animation: 'm-rise var(--dur-spatial-default) var(--ease-spatial-default)' }, '&[hidden]': { paddingBlock: '0 !important' } },
  } as any,
  variants: {
    variant: {
      default: {},
      spoiler: {
        root: {
          marginBlock: '0.9em', borderRadius: 'md', cornerShape: 'var(--corner-shape-control)', backgroundColor: 'var(--surface-quiet)',
          boxShadow: 'inset 0 0 0 1px var(--line)',
        },
        trigger: {
          display: 'flex', width: '100%', marginInline: '0', minHeight: '44px', gap: '10px', paddingInline: '12px', textAlign: 'start',
          color: 'var(--ink)', fontWeight: '500', borderRadius: 'inherit',
          '& > .m-icon:first-child': { color: 'var(--ink-muted)' },
        },
        label: { color: 'var(--ink-muted)', fontWeight: '500' },
        topic: { flex: '1', minWidth: '0', fontWeight: '600' },
        action: { color: 'var(--accent-ink)', fontWeight: '600' },
        indicator: { display: 'none' },
        content: { paddingInline: '14px', paddingBottom: '12px', paddingTop: '2px' },
      },
    },
  } as any,
  staticCss: [{ variant: ['*'] }],
})

export const tree = defineSlotRecipe({
  className: 'm-tree',
  description: 'A hierarchy a reader moves through with arrow keys: lore, an archive of arcs and scenes.',
  slots: ['root', 'item', 'row', 'toggle', 'icon', 'label', 'count', 'group'],
  base: {
    root: { margin: '0', padding: '0', listStyle: 'none', fontFamily: 'ui', fontSize: '14px', lineHeight: '20px', color: 'var(--ink)' },
    item: { outline: 'none', '&:focus-visible > .m-tree__row': { outline: '2px solid var(--focus)', outlineOffset: '-2px' } },
    row: {
      position: 'relative', display: 'flex', alignItems: 'center', gap: '6px', minHeight: '36px', paddingInlineStart: 'calc((var(--level, 1) - 1) * 20px + 6px)', paddingInlineEnd: '10px',
      borderRadius: 'md', cornerShape: 'var(--corner-shape-control)', cursor: 'pointer', userSelect: 'none', transition: motion.effects('background-color'),
      '@media (hover: hover)': { '&:hover': { backgroundColor: 'color-mix(in oklab, var(--ink) 6%, transparent)' } },
      '[aria-selected=true] > &': { backgroundColor: 'var(--accent-soft)', color: 'var(--on-accent-soft)', fontWeight: '600', '& .m-icon': { color: 'inherit' } },
      '@media (pointer: coarse)': { minHeight: '44px' },
    },
    toggle: {
      display: 'inline-grid', placeItems: 'center', width: '20px', flex: 'none', color: 'var(--ink-muted)', transition: motion.spatial('rotate'),
      '& .m-icon': { fontSize: '18px' },
      '[aria-expanded=true] > .m-tree__row > &': { rotate: '90deg' },
      '[data-leaf] > .m-tree__row > &': { visibility: 'hidden' },
    },
    icon: { fontSize: '18px !important', color: 'var(--ink-muted)' },
    label: { flex: '1', minWidth: '0', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' },
    count: { fontSize: '12.5px', color: 'var(--ink-muted)', fontVariantNumeric: 'tabular-nums', '[aria-selected=true] > .m-tree__row > &': { color: 'inherit' } },
    group: {
      position: 'relative', margin: '0', padding: '0', listStyle: 'none',
      '&::before': { content: '""', position: 'absolute', insetBlock: '2px', insetInlineStart: 'calc((var(--level, 1) - 1) * 20px + 15px)', width: '1px', backgroundColor: 'var(--line)' },
      '[aria-expanded=false] > &': { display: 'none' },
    },
  } as any,
})

export const pagination = defineSlotRecipe({
  className: 'm-pagination',
  description: 'Pages of a long Scene or list. Narrow containers show only the position and the arrows.',
  slots: ['root', 'list', 'page', 'ellipsis', 'status'],
  base: {
    root: { containerType: 'inline-size', fontFamily: 'ui' },
    list: { display: 'flex', alignItems: 'center', gap: '4px', margin: '0', padding: '0', listStyle: 'none' },
    page: {
      position: 'relative', display: 'inline-grid', placeItems: 'center', minWidth: 'var(--control-h)', height: 'var(--control-h)', paddingInline: '8px',
      border: '0', borderRadius: 'control', cornerShape: 'var(--corner-shape-control)', backgroundColor: 'transparent', color: 'var(--ink)',
      font: '500 14px/20px var(--font-ui)', fontVariantNumeric: 'tabular-nums', textDecoration: 'none', cursor: 'pointer',
      transition: motion.effects('background-color, color'), ...stateLayer, ...coarseHitArea,
      '&[aria-current=page]': { backgroundColor: 'var(--accent-soft)', color: 'var(--on-accent-soft)', fontWeight: '700' },
      '&[aria-disabled=true]': { color: 'var(--ink-muted)', pointerEvents: 'none', opacity: '0.6' },
      '& .m-icon': { fontSize: '20px' },
      '@container (max-width: 420px)': { '[data-page] > &': { display: 'none' } },
    },
    ellipsis: { display: 'inline-grid', placeItems: 'center', width: '28px', color: 'var(--ink-muted)', '@container (max-width: 420px)': { display: 'none' } },
    status: {
      display: 'none', paddingInline: '8px', fontSize: '14px', color: 'var(--ink)', fontVariantNumeric: 'tabular-nums', whiteSpace: 'nowrap',
      '@container (max-width: 420px)': { display: 'block' },
    },
  } as any,
})

export const breadcrumb = defineSlotRecipe({
  className: 'm-breadcrumb',
  description: 'Where this page sits: World, Arc, Scene; Lore, Places, an entry.',
  slots: ['root', 'list', 'item', 'link', 'separator', 'current', 'more'],
  base: {
    root: { fontFamily: 'ui', fontSize: '13px', lineHeight: '18px', minWidth: '0' },
    list: { display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '2px', margin: '0', padding: '0', listStyle: 'none' },
    item: { display: 'inline-flex', alignItems: 'center', gap: '2px', minWidth: '0', '&[hidden]': { display: 'none' } },
    link: {
      display: 'inline-flex', alignItems: 'center', gap: '4px', paddingInline: '4px', paddingBlock: '3px', borderRadius: 'sm', color: 'var(--ink-muted)', textDecoration: 'none',
      '& .m-icon': { fontSize: '16px' },
      '@media (hover: hover)': { '&:hover': { color: 'var(--ink)', textDecoration: 'underline', textUnderlineOffset: '0.2em' } },
    },
    separator: { fontSize: '16px !important', color: 'var(--ink-muted)' },
    current: { paddingInline: '4px', fontWeight: '600', color: 'var(--ink)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' },
    more: {
      position: 'relative', display: 'inline-grid', placeItems: 'center', width: '28px', height: '24px', padding: '0', border: '0', borderRadius: 'sm',
      backgroundColor: 'transparent', color: 'var(--ink-muted)', cursor: 'pointer', '& .m-icon': { fontSize: '18px' }, ...stateLayer, ...coarseHitArea,
    },
  } as any,
})

export const steps = defineSlotRecipe({
  className: 'm-steps',
  description: 'Progress through a fixed sequence: creating a character, setting up a World.',
  slots: ['root', 'list', 'step', 'marker', 'text', 'title', 'description'],
  base: {
    root: { containerType: 'inline-size', fontFamily: 'ui' },
    list: {
      display: 'grid', margin: '0', padding: '0', listStyle: 'none',
      '@container (min-width: 560px)': { gridAutoFlow: 'column', gridAutoColumns: 'minmax(0, 1fr)' },
    },
    step: {
      position: 'relative', display: 'grid', gridTemplateColumns: '28px minmax(0, 1fr)', columnGap: '12px', paddingBottom: '20px',
      '&:last-child': { paddingBottom: '0' },
      '&:not(:last-child)::after': {
        content: '""', position: 'absolute', insetInlineStart: '13px', top: '32px', bottom: '4px', width: '2px', borderRadius: '1px', backgroundColor: 'var(--line-strong)',
      },
      '&[data-state=complete]:not(:last-child)::after': { backgroundColor: 'var(--accent)' },
      '@container (min-width: 560px)': {
        gridTemplateColumns: 'minmax(0, 1fr)', rowGap: '10px', paddingBottom: '0', paddingInlineEnd: '16px',
        '&:not(:last-child)::after': { insetInlineStart: '36px', insetInlineEnd: '8px', top: '13px', bottom: 'auto', width: 'auto', height: '2px' },
      },
    },
    marker: {
      display: 'grid', placeItems: 'center', width: '28px', height: '28px', borderRadius: '50%', border: '2px solid var(--line-strong)',
      backgroundColor: 'var(--surface)', color: 'var(--ink-muted)', fontSize: '13px', fontWeight: '700', fontVariantNumeric: 'tabular-nums',
      '& .m-icon': { fontSize: '18px' },
      '[data-state=complete] > &': { backgroundColor: 'var(--accent)', borderColor: 'var(--accent)', color: 'var(--on-accent)' },
      '[data-state=current] > &': { borderColor: 'var(--accent)', color: 'var(--accent-ink)', boxShadow: '0 0 0 4px var(--accent-soft)' },
    },
    text: { display: 'grid', gap: '2px', paddingTop: '3px', minWidth: '0' },
    title: { fontSize: '14px', lineHeight: '20px', fontWeight: '600', color: 'var(--ink)', '[data-state=upcoming] &': { fontWeight: '500' } },
    description: { fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)' },
  } as any,
  variants: {
    orientation: {
      auto: {},
      vertical: { list: { '@container (min-width: 560px)': { gridAutoFlow: 'row' } }, step: { '@container (min-width: 560px)': { gridTemplateColumns: '28px minmax(0, 1fr)', rowGap: '0', paddingBottom: '20px', '&:not(:last-child)::after': { insetInlineStart: '13px', insetInlineEnd: 'auto', top: '32px', bottom: '4px', width: '2px', height: 'auto' } } } },
    },
  } as any,
  staticCss: [{ orientation: ['*'] }],
})
