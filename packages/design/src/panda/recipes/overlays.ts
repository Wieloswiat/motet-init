import { defineSlotRecipe } from '@pandacss/dev'
import { popupSurface } from '../tokens'

/* Overlays beyond menus and dialogs. Popovers and hover cards use the frosted material in the
   top layer; the drawer is a modal <dialog>; toasts are a third chrome island, so they keep
   Motet's styling inside every World. */

export const popover = defineSlotRecipe({
  className: 'm-popover',
  description: 'A small non-modal panel anchored to the control that opened it.',
  slots: ['root', 'header', 'title', 'body', 'footer'],
  base: {
    root: { ...popupSurface, padding: '0', width: 'var(--popover-w, 320px)', maxWidth: 'calc(100vw - 24px)' },
    header: { display: 'flex', alignItems: 'center', gap: '8px', paddingInline: '16px 6px', paddingBlock: '8px 0' },
    title: { flex: '1', margin: '0', fontSize: '15px', lineHeight: '22px', fontWeight: '600' },
    body: { paddingInline: '16px', paddingBlock: '8px 16px', fontSize: '14px', lineHeight: '21px', '& > p': { margin: '0 0 8px', color: 'var(--ink-muted)' } },
    footer: { display: 'flex', justifyContent: 'flex-end', gap: '8px', paddingInline: '12px', paddingBottom: '12px' },
  } as any,
})

export const hovercard = defineSlotRecipe({
  className: 'm-hovercard',
  description: 'A preview of a character or account while pointing at its name. Never the only route to the information.',
  slots: ['root', 'head', 'name', 'epithet', 'body', 'owner'],
  base: {
    root: { ...popupSurface, padding: '16px', width: '300px', pointerEvents: 'auto' },
    head: { display: 'grid', gridTemplateColumns: 'auto minmax(0, 1fr)', columnGap: '12px', alignItems: 'center' },
    name: {
      display: 'block', fontFamily: 'name', fontSize: 'calc(17px * var(--name-adjust, 1))', lineHeight: '1.25', fontWeight: '600',
      textTransform: 'var(--name-case, none)', letterSpacing: 'var(--name-tracking, 0)', fontVariationSettings: 'var(--name-axes, normal)',
    },
    epithet: { display: 'block', marginTop: '2px', fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)' },
    body: { marginTop: '12px', fontSize: '13.5px', lineHeight: '20px', color: 'var(--ink)' },
    owner: { display: 'flex', alignItems: 'center', gap: '6px', marginTop: '12px', paddingTop: '10px', boxShadow: 'inset 0 1px 0 var(--menu-line, var(--line))', fontSize: '13px', color: 'var(--ink-muted)' },
  } as any,
})

export const drawer = defineSlotRecipe({
  className: 'm-drawer',
  description: 'A modal panel from the side (a bottom sheet on phones) for secondary work that keeps its context.',
  slots: ['root', 'header', 'title', 'body', 'footer'],
  base: {
    root: {
      position: 'fixed', inset: '0 0 0 auto', margin: '0', width: 'min(420px, calc(100vw - 24px))', height: '100dvh', maxWidth: 'none', maxHeight: 'none',
      padding: '0', border: '0', borderInlineStart: '1px solid var(--line)', borderRadius: 'var(--radius-xl) 0 0 var(--radius-xl)', cornerShape: 'var(--corner-shape)',
      backgroundColor: 'var(--surface-raised)', color: 'var(--ink)', boxShadow: 'var(--elevation-4)', fontFamily: 'ui', overflow: 'hidden',
      '&[open]': { display: 'flex', flexDirection: 'column', animation: 'm-drawer-in var(--dur-spatial-default) var(--ease-spatial-default)' },
      '&::backdrop': { backgroundColor: 'var(--scrim)', animation: 'm-fade-in var(--dur-effects-default) var(--ease-effects-default)' },
      '@media (max-width: 640px)': {
        inset: 'auto 0 0 0', width: '100%', height: 'auto', maxHeight: '85dvh', borderInlineStart: '0', borderTop: '1px solid var(--line)',
        borderRadius: 'var(--radius-xl) var(--radius-xl) 0 0',
        '&[open]': { animation: 'm-sheet-in var(--dur-spatial-default) var(--ease-spatial-default)' },
      },
    },
    header: { display: 'flex', alignItems: 'center', gap: '8px', paddingInline: '20px 10px', paddingBlock: '12px', boxShadow: 'inset 0 -1px 0 var(--line)' },
    title: { flex: '1', margin: '0', fontSize: '18px', lineHeight: '24px', fontWeight: '600' },
    body: { flex: '1', overflowY: 'auto', overscrollBehavior: 'contain', padding: '20px' },
    footer: { display: 'flex', justifyContent: 'flex-end', flexWrap: 'wrap', gap: '8px', paddingInline: '16px', paddingBlock: '12px', boxShadow: 'inset 0 1px 0 var(--line)' },
  } as any,
})

export const toast = defineSlotRecipe({
  className: 'm-toast',
  description: 'A short platform message about something that just happened, with at most one action.',
  slots: ['region', 'root', 'icon', 'message', 'action', 'close'],
  base: {
    region: {
      position: 'fixed', insetInlineStart: '12px', bottom: '12px', zIndex: '65', display: 'flex', flexDirection: 'column', gap: '8px',
      width: 'min(420px, calc(100vw - 24px))', pointerEvents: 'none', margin: '0', padding: '0', listStyle: 'none',
    },
    root: {
      pointerEvents: 'auto', display: 'flex', alignItems: 'center', gap: '10px', minHeight: '50px', paddingInline: '14px 6px', paddingBlock: '6px',
      backgroundColor: 'var(--chrome-bg)', backgroundImage: 'var(--chrome-grain)', backdropFilter: 'blur(var(--chrome-blur)) saturate(var(--chrome-saturate))',
      border: '1px solid var(--chrome-line)', borderRadius: '18px', cornerShape: 'squircle', color: 'var(--chrome-ink)',
      boxShadow: 'var(--chrome-shadow), var(--elevation-3)', fontFamily: 'var(--font-ui)', fontSize: '14px', lineHeight: '20px',
      animation: 'm-rise var(--dur-spatial-default) var(--ease-spatial-default)',
      transition: 'opacity var(--dur-effects-default) var(--ease-effects-default), translate var(--dur-effects-default) var(--ease-effects-default)',
      '&[data-leaving]': { opacity: '0', translate: '0 8px' },
      '@supports (corner-shape: squircle)': { borderRadius: '24px' },
      '@media (prefers-reduced-transparency: reduce)': { backgroundColor: 'var(--chrome-bg-solid)', backdropFilter: 'none' },
    },
    icon: {
      color: 'var(--chrome-ink-muted)',
      '[data-tone=positive] > &': { color: 'var(--positive-ink)' }, '[data-tone=caution] > &': { color: 'var(--caution-ink)' },
      '[data-tone=danger] > &': { color: 'var(--danger-ink)' }, '[data-tone=info] > &': { color: 'var(--info-ink)' },
    },
    message: { flex: '1', minWidth: '0', paddingBlock: '4px' },
    action: {
      flex: 'none', minHeight: '36px', paddingInline: '10px', border: '0', borderRadius: '12px', backgroundColor: 'transparent',
      color: 'var(--chrome-accent)', font: '600 14px/20px var(--font-ui)', cursor: 'pointer',
      '@media (hover: hover)': { '&:hover': { backgroundColor: 'var(--chrome-hover)' } },
    },
    close: { color: 'var(--chrome-ink-muted)' },
  } as any,
})
