import { defineSlotRecipe } from '@pandacss/dev'
import { entryBox, motion, optionRow, popupSurface } from '../tokens'
import { coarseHitArea, stateLayer } from './controls'

/* Form controls. Native inputs wherever the platform's semantics are right (checkbox, radio,
   range, time, file); custom listboxes and a calendar where native popups can't wear a World.
   Controls take the World's colour and corner, clamped; they never take a frame profile. */

const svg = (body: string) => `url("data:image/svg+xml,${encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 20 20'>${body}</svg>`)}")`
const CHECK = svg("<path d='M4.6 10.4l3.6 3.6 7.2-8' fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'/>")
const DASH = svg("<path d='M5 10h10' stroke='black' stroke-width='2.4' stroke-linecap='round'/>")

/* The row a checkbox or radio sits in: control, then label and description. The whole row is the label. */
const choiceRow = {
  position: 'relative', display: 'grid', gridTemplateColumns: 'auto minmax(0, 1fr)', columnGap: '12px', alignItems: 'start',
  paddingBlock: '8px', cursor: 'pointer', fontFamily: 'ui', color: 'var(--ink)', WebkitTapHighlightColor: 'transparent',
  '&:has(:disabled)': { cursor: 'not-allowed', color: 'var(--ink-muted)' },
} as const
const choiceHalo = {
  '&::before': {
    content: '""', position: 'absolute', inset: '-10px', borderRadius: '50%', backgroundColor: 'transparent',
    transition: motion.effects('background-color'),
  },
} as const
const choiceText = {
  text: { display: 'grid', gap: '2px', minWidth: '0' },
  label: { fontSize: '14px', lineHeight: '20px' },
  description: { fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)' },
}

export const checkbox = defineSlotRecipe({
  className: 'm-checkbox',
  description: 'A choice that is on or off, or a set of independent choices in a fieldset.',
  slots: ['root', 'control', 'text', 'label', 'description'],
  base: {
    root: choiceRow,
    control: {
      appearance: 'none', position: 'relative', flex: 'none', width: '20px', height: '20px', margin: '0',
      border: '2px solid var(--line-strong)', borderRadius: 'min(var(--radius-xs), 6px)', cornerShape: 'var(--corner-shape-control)',
      backgroundColor: 'var(--surface-raised)', cursor: 'inherit', transition: motion.effects('background-color, border-color'),
      ...choiceHalo,
      '&::after': {
        content: '""', position: 'absolute', inset: '-2px', backgroundColor: 'var(--on-accent)', maskImage: CHECK, maskSize: '100% 100%',
        scale: '0.4', opacity: '0', transition: `${motion.spatial('scale')}, ${motion.effects('opacity')}`,
      },
      '&:checked, &:indeterminate': { backgroundColor: 'var(--accent)', borderColor: 'var(--accent)', '&::after': { scale: '1', opacity: '1' } },
      '&:indeterminate::after': { maskImage: DASH },
      '.m-checkbox__root:hover &:not(:disabled)::before': { backgroundColor: 'color-mix(in oklab, var(--accent) 10%, transparent)' },
      '&:focus-visible': { outline: '2px solid var(--focus)', outlineOffset: '2px' },
      '&[aria-invalid=true]': { borderColor: 'var(--danger-ink)' },
      '&:disabled': { borderColor: 'var(--line)', backgroundColor: 'var(--surface-quiet)' },
      '&:disabled:checked, &:disabled:indeterminate': { backgroundColor: 'var(--ink-muted)', borderColor: 'transparent' },
    },
    ...choiceText,
  } as any,
})

export const radio = defineSlotRecipe({
  className: 'm-radio',
  description: 'One choice from a short visible set. Cards when each choice needs a sentence.',
  slots: ['root', 'item', 'control', 'icon', 'text', 'label', 'description'],
  base: {
    root: { display: 'grid', gap: '0' },
    item: choiceRow,
    control: {
      /* radios stay round in every World: the circle is the meaning */
      appearance: 'none', position: 'relative', flex: 'none', width: '20px', height: '20px', margin: '0',
      border: '2px solid var(--line-strong)', borderRadius: '50%', cornerShape: 'round', backgroundColor: 'var(--surface-raised)', cursor: 'inherit',
      transition: motion.effects('border-color'),
      ...choiceHalo,
      '&::after': {
        content: '""', position: 'absolute', inset: '3px', borderRadius: '50%', backgroundColor: 'var(--accent)',
        scale: '0', transition: motion.spatial('scale'),
      },
      '&:checked': { borderColor: 'var(--accent)', '&::after': { scale: '1' } },
      '.m-radio__item:hover &:not(:disabled)::before': { backgroundColor: 'color-mix(in oklab, var(--accent) 10%, transparent)' },
      '&:focus-visible': { outline: '2px solid var(--focus)', outlineOffset: '2px' },
      '&:disabled': { borderColor: 'var(--line)', '&::after': { backgroundColor: 'var(--ink-muted)' } },
    },
    icon: { display: 'none' },
    ...choiceText,
  } as any,
  variants: {
    variant: {
      list: {},
      cards: {
        root: { gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '8px' },
        item: {
          gridTemplateColumns: 'auto minmax(0, 1fr) auto', paddingBlock: '12px', paddingInline: '14px',
          border: '1px solid var(--line-strong)', borderRadius: 'md', cornerShape: 'var(--corner-shape-control)', backgroundColor: 'var(--surface-raised)',
          transition: motion.effects('border-color, background-color, box-shadow'),
          '&:has(:checked)': { borderColor: 'var(--accent)', boxShadow: 'inset 0 0 0 1px var(--accent)', backgroundColor: 'color-mix(in oklab, var(--accent-soft) 45%, var(--surface-raised))' },
          '&:has(:focus-visible)': { outline: '2px solid var(--focus)', outlineOffset: '2px' },
          '&:hover:not(:has(:disabled))': { borderColor: 'var(--accent-ink)' },
        },
        control: { '&::before': { display: 'none' }, '&:focus-visible': { outline: 'none' } },
        icon: { display: 'block', fontSize: '22px', color: 'var(--ink-muted)', '.m-radio__item:has(:checked) &': { color: 'var(--accent-ink)' } },
        label: { fontWeight: '600' },
      },
    },
  } as any,
  staticCss: [{ variant: ['*'] }],
})

export const fieldset = defineSlotRecipe({
  className: 'm-fieldset',
  description: 'A labelled group of related controls.',
  slots: ['root', 'legend', 'help', 'content'],
  base: {
    root: { minWidth: '0', margin: '0', padding: '0', border: '0', fontFamily: 'ui' },
    legend: { padding: '0', fontSize: '14px', lineHeight: '20px', fontWeight: '600', color: 'var(--ink)' },
    help: { marginTop: '2px', fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)' },
    content: { display: 'grid', marginTop: '6px' },
  } as any,
})

/* Text entry with leading or trailing parts: icons, units, a time zone, a reveal button, steppers. */
export const input = defineSlotRecipe({
  className: 'm-input',
  description: 'The shared text-entry box: one border, one focus ring, optional addons and actions.',
  slots: ['root', 'control', 'addon', 'action'],
  base: {
    root: entryBox,
    control: {
      flex: '1', minWidth: '0', alignSelf: 'stretch', paddingInline: '12px', border: '0', margin: '0', backgroundColor: 'transparent',
      color: 'inherit', font: 'inherit', borderRadius: 'inherit', outline: 'none',
      '&::placeholder': { color: 'var(--ink-muted)', opacity: '1' },
      '.m-input__addon[data-side=start] + &': { paddingInlineStart: '6px' },
      '&[type=time], &[inputmode=decimal], &[inputmode=numeric]': { fontVariantNumeric: 'tabular-nums' },
      '&::-webkit-calendar-picker-indicator': { display: 'none' },
      '&::-webkit-datetime-edit-fields-wrapper': { padding: '0' },
      '&::-ms-reveal': { display: 'none' },
    },
    addon: {
      flex: 'none', display: 'inline-flex', alignItems: 'center', gap: '4px', color: 'var(--ink-muted)', fontSize: '13px', whiteSpace: 'nowrap',
      '&[data-side=start]': { paddingInlineStart: '10px' },
      '&[data-side=end]': { paddingInlineEnd: '12px' },
      '& .m-icon': { fontSize: '20px' },
    },
    action: {
      flex: 'none', display: 'inline-flex', alignItems: 'center', marginInline: '3px',
      '& .m-icon-button': { width: '32px', height: '32px', '& .m-icon': { fontSize: '18px' } },
    },
  } as any,
})

export const textarea = defineSlotRecipe({
  className: 'm-textarea',
  description: 'Multi-line text that grows with its content up to a limit.',
  slots: ['root', 'control', 'footer', 'counter'],
  base: {
    root: { display: 'grid', gap: '6px', minWidth: '0' },
    control: {
      ...entryBox, display: 'block', paddingInline: '12px', paddingBlock: '9px', resize: 'vertical',
      fieldSizing: 'content', minHeight: 'calc(var(--rows, 3) * 1.55em + 20px)', maxHeight: 'calc(var(--max-rows, 12) * 1.55em + 20px)',
      '&:focus': { borderColor: 'var(--accent-ink)', outline: '2px solid var(--focus)', outlineOffset: '1px' },
      '&::placeholder': { color: 'var(--ink-muted)', opacity: '1' },
      '&[aria-invalid=true]': { borderColor: 'var(--danger-ink)' },
    },
    footer: { display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: '12px', fontFamily: 'ui', fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)' },
    counter: {
      marginInlineStart: 'auto', fontVariantNumeric: 'tabular-nums', whiteSpace: 'nowrap',
      '&[data-state=near]': { color: 'var(--caution-ink)' },
      '&[data-state=over]': { color: 'var(--danger-ink)', fontWeight: '600' },
    },
  } as any,
  variants: {
    face: {
      reading: { control: { fontFamily: 'reading', fontSize: 'calc(16px * var(--text-scale, 1))', lineHeight: '1.55' } },
      ui: { control: { lineHeight: '1.45' } },
    },
  } as any,
  staticCss: [{ face: ['*'] }],
})

/* Listbox: the popup of Select, Combobox and tag suggestions */
export const listbox = defineSlotRecipe({
  className: 'm-listbox',
  slots: ['root', 'group', 'groupLabel', 'option', 'optionText', 'optionLabel', 'optionDescription', 'check', 'empty'],
  base: {
    root: { ...popupSurface, minWidth: '200px', maxHeight: 'min(320px, 50vh)', overscrollBehavior: 'contain' },
    group: { '& + &': { marginTop: '4px', paddingTop: '4px', boxShadow: 'inset 0 1px 0 var(--menu-line, var(--line))' } },
    groupLabel: { paddingInline: '10px', paddingBlock: '6px 2px', fontSize: '12.5px', lineHeight: '16px', fontWeight: '600', color: 'var(--menu-ink-muted, var(--ink-muted))' },
    option: optionRow,
    optionText: { display: 'grid', minWidth: '0', flex: '1' },
    optionLabel: { overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', '& mark': { backgroundColor: 'transparent', color: 'inherit', fontWeight: '700', textDecoration: 'underline', textDecorationColor: 'var(--accent-ink)', textUnderlineOffset: '0.2em' } },
    optionDescription: { fontSize: '12.5px', lineHeight: '17px', color: 'var(--menu-ink-muted, var(--ink-muted))', fontWeight: '400' },
    check: { marginInlineStart: 'auto', visibility: 'hidden', color: 'var(--accent-ink)', '[aria-selected=true] > &': { visibility: 'visible' } },
    empty: { paddingInline: '10px', paddingBlock: '10px', color: 'var(--menu-ink-muted, var(--ink-muted))' },
  } as any,
})

export const select = defineSlotRecipe({
  className: 'm-select',
  description: 'Pick one value from a list too long for radios.',
  slots: ['root', 'trigger', 'value', 'indicator'],
  base: {
    root: { position: 'relative', display: 'grid', gap: '6px', minWidth: '0' },
    trigger: {
      ...entryBox, gap: '8px', paddingInline: '12px 8px', textAlign: 'start', cursor: 'pointer',
      '&:focus-visible': { borderColor: 'var(--accent-ink)', outline: '2px solid var(--focus)', outlineOffset: '1px' },
      '&:focus:not(:focus-visible)': { outline: 'none' },
      '&[aria-expanded=true]': { borderColor: 'var(--accent-ink)' },
      '& > .m-icon:first-child': { color: 'var(--ink-muted)' },
      '&:disabled': { cursor: 'not-allowed', backgroundColor: 'var(--surface-quiet)', color: 'var(--ink-muted)' },
    },
    value: { flex: '1', minWidth: '0', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', '&[data-placeholder]': { color: 'var(--ink-muted)' } },
    indicator: { color: 'var(--ink-muted)', transition: motion.spatial('rotate'), '[aria-expanded=true] > &': { rotate: '180deg' } },
  } as any,
})

export const tags = defineSlotRecipe({
  className: 'm-tags',
  description: 'A short list of free-form or suggested values: content notes, scene tags.',
  slots: ['root', 'tag', 'remove', 'control'],
  base: {
    root: { ...entryBox, flexWrap: 'wrap', gap: '4px', padding: '4px', cursor: 'text' },
    tag: {
      display: 'inline-flex', alignItems: 'center', gap: '2px', minHeight: '28px', paddingInline: '8px 2px', maxWidth: '100%',
      borderRadius: 'sm', cornerShape: 'var(--corner-shape-control)', backgroundColor: 'var(--surface-quiet)', boxShadow: 'inset 0 0 0 1px var(--line)',
      fontSize: '13px', lineHeight: '18px', fontWeight: '500', color: 'var(--ink)', overflowWrap: 'anywhere',
      '&[data-highlighted]': { backgroundColor: 'var(--accent-soft)', color: 'var(--on-accent-soft)', boxShadow: 'inset 0 0 0 1px var(--accent-ink)' },
    },
    remove: {
      position: 'relative', display: 'inline-grid', placeItems: 'center', width: '24px', height: '24px', padding: '0', border: '0',
      borderRadius: 'xs', backgroundColor: 'transparent', color: 'var(--ink-muted)', cursor: 'pointer',
      '& .m-icon': { fontSize: '16px' }, ...stateLayer, ...coarseHitArea,
      '&:focus-visible': { outline: '2px solid var(--focus)', outlineOffset: '0' },
    },
    control: {
      flex: '1 1 120px', minWidth: '120px', height: '30px', paddingInline: '8px', border: '0', margin: '0',
      backgroundColor: 'transparent', color: 'inherit', font: 'inherit', outline: 'none',
      '&::placeholder': { color: 'var(--ink-muted)', opacity: '1' },
    },
  } as any,
})

export const calendar = defineSlotRecipe({
  className: 'm-calendar',
  description: 'A month grid for picking a date.',
  slots: ['root', 'header', 'title', 'grid', 'weekday', 'day', 'footer', 'popup'],
  base: {
    root: { width: 'max-content', fontFamily: 'ui', color: 'var(--ink)' },
    header: { display: 'flex', alignItems: 'center', gap: '4px', marginBottom: '6px', paddingInlineStart: '8px' },
    title: { flex: '1', margin: '0', fontSize: '14px', lineHeight: '20px', fontWeight: '600', textTransform: 'capitalize' },
    grid: { borderCollapse: 'separate', borderSpacing: '2px' },
    weekday: {
      width: '36px', height: '28px', fontSize: '12px', fontWeight: '600', color: 'var(--ink-muted)', textAlign: 'center',
      '& abbr': { textDecoration: 'none' },
    },
    day: {
      position: 'relative', width: '36px', height: '36px', padding: '0', textAlign: 'center', verticalAlign: 'middle', cursor: 'pointer',
      borderRadius: 'control', cornerShape: 'var(--corner-shape-control)', fontSize: '13.5px', fontVariantNumeric: 'tabular-nums',
      transition: motion.effects('background-color, color'),
      '@media (hover: hover)': { '&:hover:not([aria-disabled=true])': { backgroundColor: 'color-mix(in oklab, var(--ink) 8%, transparent)' } },
      '&[data-outside]': { color: 'var(--ink-muted)' },
      '&[aria-current=date]': { color: 'var(--accent-ink)', fontWeight: '700', boxShadow: 'inset 0 0 0 1px var(--accent-ink)' },
      '&[aria-selected=true]': { backgroundColor: 'var(--accent) !important', color: 'var(--on-accent)', fontWeight: '700', boxShadow: 'none' },
      '&[aria-disabled=true]': { opacity: '0.42', cursor: 'not-allowed' },
      '&:focus-visible': { outline: '2px solid var(--focus)', outlineOffset: '0' },
      '&:focus': { outlineOffset: '0' },
    },
    footer: { display: 'flex', justifyContent: 'space-between', gap: '8px', marginTop: '6px', paddingTop: '6px', boxShadow: 'inset 0 1px 0 var(--menu-line, var(--line))' },
    popup: { ...popupSurface, padding: '10px' },
  } as any,
})

export const slider = defineSlotRecipe({
  className: 'm-slider',
  description: 'A value on a continuous or stepped range, with its value always printed.',
  slots: ['root', 'header', 'label', 'value', 'control', 'marks'],
  base: {
    root: { display: 'grid', gap: '4px', fontFamily: 'ui', minWidth: '0' },
    header: { display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', gap: '12px' },
    label: { fontSize: '13px', lineHeight: '18px', fontWeight: '600', color: 'var(--ink)' },
    value: { fontSize: '13px', lineHeight: '18px', color: 'var(--ink)', fontVariantNumeric: 'tabular-nums' },
    control: {
      '--fill': '50%', appearance: 'none', width: '100%', height: '28px', margin: '0', backgroundColor: 'transparent', cursor: 'pointer',
      '&::-webkit-slider-runnable-track': {
        height: '6px', borderRadius: '3px', boxShadow: 'inset 0 0 0 1px var(--line)',
        background: 'linear-gradient(to right, var(--accent) var(--fill), var(--surface-quiet) var(--fill))',
      },
      '&::-webkit-slider-thumb': {
        appearance: 'none', width: '20px', height: '20px', marginTop: '-7px', borderRadius: '50%', border: '2px solid var(--accent)',
        backgroundColor: 'var(--surface-raised)', boxShadow: 'var(--elevation-1)', transition: motion.spatial('scale'),
      },
      '&:active::-webkit-slider-thumb': { scale: '1.15' },
      '&::-moz-range-track': { height: '6px', borderRadius: '3px', backgroundColor: 'var(--surface-quiet)', boxShadow: 'inset 0 0 0 1px var(--line)' },
      '&::-moz-range-progress': { height: '6px', borderRadius: '3px', backgroundColor: 'var(--accent)' },
      '&::-moz-range-thumb': { width: '16px', height: '16px', borderRadius: '50%', border: '2px solid var(--accent)', backgroundColor: 'var(--surface-raised)', boxShadow: 'var(--elevation-1)' },
      '&:focus-visible': {
        outline: 'none',
        '&::-webkit-slider-thumb': { outline: '2px solid var(--focus)', outlineOffset: '2px' },
        '&::-moz-range-thumb': { outline: '2px solid var(--focus)', outlineOffset: '2px' },
      },
      '&:disabled': { cursor: 'not-allowed', '&::-webkit-slider-thumb': { borderColor: 'var(--ink-muted)' }, '--accent': 'var(--ink-muted)' },
    },
    marks: { display: 'flex', justifyContent: 'space-between', fontSize: '12px', lineHeight: '16px', color: 'var(--ink-muted)', fontVariantNumeric: 'tabular-nums' },
  } as any,
})

export const upload = defineSlotRecipe({
  className: 'm-upload',
  description: 'Add files by dropping them or choosing them; each file shows its own state.',
  slots: ['root', 'dropzone', 'icon', 'title', 'hint', 'list', 'item', 'thumb', 'name', 'meta', 'remove'],
  base: {
    root: { display: 'grid', gap: '10px', fontFamily: 'ui', minWidth: '0' },
    dropzone: {
      position: 'relative', display: 'grid', justifyItems: 'center', gap: '6px', paddingInline: '16px', paddingBlock: '22px', textAlign: 'center',
      border: '1.5px dashed var(--line-strong)', borderRadius: 'lg', cornerShape: 'var(--corner-shape)', color: 'var(--ink)', cursor: 'pointer',
      transition: motion.effects('border-color, background-color'),
      '&[data-dragover]': { borderStyle: 'solid', borderColor: 'var(--accent-ink)', backgroundColor: 'color-mix(in oklab, var(--accent-soft) 60%, transparent)' },
      '&:has(:focus-visible)': { outline: '2px solid var(--focus)', outlineOffset: '2px' },
      '& input[type=file]': { position: 'absolute', width: '1px', height: '1px', opacity: '0', pointerEvents: 'none' },
    },
    icon: { fontSize: '28px !important', color: 'var(--ink-muted)' },
    title: { fontSize: '14px', lineHeight: '20px', fontWeight: '600' },
    hint: { fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)' },
    list: { display: 'grid', gap: '6px', margin: '0', padding: '0', listStyle: 'none' },
    item: {
      display: 'grid', gridTemplateColumns: '40px minmax(0, 1fr) auto', alignItems: 'center', columnGap: '12px', padding: '8px',
      border: '1px solid var(--line)', borderRadius: 'md', cornerShape: 'var(--corner-shape-control)', backgroundColor: 'var(--surface-raised)',
      '&[data-state=error]': { borderColor: 'var(--danger-ink)' },
    },
    thumb: {
      width: '40px', height: '40px', overflow: 'hidden', display: 'grid', placeItems: 'center', borderRadius: 'sm', cornerShape: 'var(--corner-shape-control)',
      backgroundColor: 'var(--surface-quiet)', color: 'var(--ink-muted)', '& img': { width: '100%', height: '100%', objectFit: 'cover' },
    },
    name: { fontSize: '14px', lineHeight: '20px', fontWeight: '500', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' },
    meta: {
      display: 'flex', alignItems: 'center', gap: '4px', fontSize: '12.5px', lineHeight: '17px', color: 'var(--ink-muted)', fontVariantNumeric: 'tabular-nums',
      '[data-state=error] &': { color: 'var(--danger-ink)' }, '& .m-icon': { fontSize: '16px' },
      '& .m-progress__track': { flex: '1', maxWidth: '160px' },
    },
    remove: {},
  } as any,
})

/* OKLCH colour picker: a lightness × chroma plane at one hue with sRGB's edge drawn in, a hue
   track, numeric L C H, and contrast against the grounds the colour has to work on. */
export const colorpicker = defineSlotRecipe({
  className: 'm-colorpicker',
  description: 'Pick a colour in OKLCH with its gamut and contrast duties in view.',
  slots: ['root', 'control', 'trigger', 'swatch', 'hex', 'panel', 'area', 'canvas', 'thumb', 'hue', 'values', 'value', 'valueLabel', 'valueInput', 'note', 'checks', 'check', 'sample', 'checkLabel', 'ratio', 'status', 'swatches', 'swatchButton'],
  base: {
    root: { display: 'grid', gap: '6px', fontFamily: 'ui', minWidth: '0', '--pick': '#808080' },
    control: { '& .m-input__addon[data-side=start]': { paddingInlineStart: '5px' } },
    trigger: {
      position: 'relative', display: 'grid', placeItems: 'center', width: '30px', height: '30px', padding: '0', border: '0', borderRadius: 'sm',
      cornerShape: 'var(--corner-shape-control)', backgroundColor: 'transparent', cursor: 'pointer', ...stateLayer,
      '&:focus-visible': { outline: '2px solid var(--focus)', outlineOffset: '1px' },
      ...coarseHitArea,
    },
    swatch: {
      width: '22px', height: '22px', borderRadius: 'xs', cornerShape: 'var(--corner-shape-control)', backgroundColor: 'var(--pick)',
      boxShadow: 'inset 0 0 0 1px color-mix(in oklab, var(--ink) 22%, transparent)',
      '[data-empty] &': {
        backgroundColor: 'transparent',
        backgroundImage: 'linear-gradient(135deg, transparent calc(50% - 0.75px), var(--ink-muted) calc(50% - 0.75px) calc(50% + 0.75px), transparent calc(50% + 0.75px))',
      },
    },
    hex: { fontFamily: 'mono', fontSize: '13.5px', letterSpacing: '0', paddingInlineStart: '6px !important' },
    panel: {
      ...popupSurface, width: '300px', maxWidth: 'calc(100vw - 16px)', padding: '12px', display: 'grid', gap: '12px', overflow: 'visible',
      '&[data-static]': {
        position: 'static', width: 'auto', maxWidth: '360px', backdropFilter: 'none', boxShadow: 'none', backgroundColor: 'var(--surface-raised)',
        border: '1px solid var(--line)',
      },
      '&[popover]:not(:popover-open)': { display: 'none' },
    },
    area: {
      position: 'relative', width: '100%', aspectRatio: '16 / 10', touchAction: 'none', cursor: 'crosshair', userSelect: 'none',
      '&[data-dragging]': { cursor: 'none' },
    },
    canvas: {
      display: 'block', width: '100%', height: '100%', borderRadius: 'sm', cornerShape: 'var(--corner-shape-control)',
      boxShadow: 'inset 0 0 0 1px color-mix(in oklab, var(--ink) 16%, transparent)',
    },
    thumb: {
      position: 'absolute', width: '18px', height: '18px', marginLeft: '-9px', marginTop: '-9px', borderRadius: '50%', backgroundColor: 'var(--pick)',
      border: '2px solid #fff', boxShadow: '0 0 0 1px rgb(0 0 0 / 0.55), 0 1px 4px rgb(0 0 0 / 0.35)', pointerEvents: 'none',
      transition: motion.effects('transform'),
      '&[data-tone=light]': { borderColor: '#141414', boxShadow: '0 0 0 1px rgb(255 255 255 / 0.7), 0 1px 4px rgb(0 0 0 / 0.25)' },
      '[data-dragging] &': { transform: 'scale(1.15)' },
      '&:focus-visible': { outline: '2px solid var(--focus)', outlineOffset: '3px' },
    },
    hue: {
      '--track': 'linear-gradient(to right, red, red)',
      appearance: 'none', display: 'block', width: '100%', height: '24px', margin: '0', backgroundColor: 'transparent', cursor: 'pointer',
      '&::-webkit-slider-runnable-track': { height: '12px', borderRadius: '6px', background: 'var(--track)', boxShadow: 'inset 0 0 0 1px color-mix(in oklab, var(--ink) 16%, transparent)' },
      '&::-moz-range-track': { height: '12px', borderRadius: '6px', background: 'var(--track)' },
      '&::-webkit-slider-thumb': {
        appearance: 'none', width: '18px', height: '18px', marginTop: '-3px', borderRadius: '50%', backgroundColor: 'var(--pick)',
        border: '2px solid #fff', boxShadow: '0 0 0 1px rgb(0 0 0 / 0.55), 0 1px 4px rgb(0 0 0 / 0.35)',
      },
      '&::-moz-range-thumb': { width: '14px', height: '14px', borderRadius: '50%', backgroundColor: 'var(--pick)', border: '2px solid #fff', boxShadow: '0 0 0 1px rgb(0 0 0 / 0.55)' },
      '&:focus-visible': { outline: 'none', '&::-webkit-slider-thumb': { outline: '2px solid var(--focus)', outlineOffset: '2px' }, '&::-moz-range-thumb': { outline: '2px solid var(--focus)', outlineOffset: '2px' } },
    },
    values: { display: 'grid', gridTemplateColumns: 'repeat(3, minmax(0, 1fr))', gap: '8px', '&[data-count="4"] > :first-child': { gridColumn: '1 / -1' } },
    value: {
      display: 'flex', alignItems: 'center', minWidth: '0', height: '32px', border: '1px solid var(--line-strong)', borderRadius: 'control',
      cornerShape: 'var(--corner-shape-control)', backgroundColor: 'var(--surface-raised)', cursor: 'text',
      '&:focus-within': { borderColor: 'var(--accent-ink)', outline: '2px solid var(--focus)', outlineOffset: '1px' },
    },
    valueLabel: { flex: 'none', paddingInlineStart: '8px', fontSize: '12px', fontWeight: '600', color: 'var(--ink-muted)' },
    valueInput: {
      flex: '1', minWidth: '0', width: '100%', height: '100%', paddingInline: '6px', border: '0', backgroundColor: 'transparent', color: 'var(--ink)',
      fontFamily: 'mono', fontSize: '12.5px', fontVariantNumeric: 'tabular-nums', outline: 'none', borderRadius: 'inherit',
    },
    note: { display: 'flex', gap: '6px', alignItems: 'flex-start', fontSize: '12.5px', lineHeight: '17px', color: 'var(--ink-muted)', '& .m-icon': { fontSize: '16px', marginTop: '0.5px' } },
    checks: { display: 'grid', gap: '4px', margin: '0', padding: '0', listStyle: 'none', '&[hidden]': { display: 'none' } },
    check: {
      display: 'grid', gridTemplateColumns: '40px minmax(0, 1fr) auto', gridTemplateAreas: '"s l r" "s t t"', alignItems: 'center', columnGap: '10px',
      fontSize: '13px', lineHeight: '18px',
    },
    sample: {
      gridArea: 's', display: 'grid', placeItems: 'center', height: '30px', borderRadius: 'xs', cornerShape: 'var(--corner-shape-control)',
      backgroundColor: 'var(--g)', color: 'var(--pick)', fontWeight: '700', fontSize: '14px', boxShadow: 'inset 0 0 0 1px color-mix(in oklab, var(--ink) 14%, transparent)',
    },
    checkLabel: { gridArea: 'l', color: 'var(--ink)', minWidth: '0', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' },
    ratio: { gridArea: 'r', fontFamily: 'mono', fontSize: '12.5px', fontVariantNumeric: 'tabular-nums', color: 'var(--ink)', '[data-pass=false] &': { color: 'var(--danger-ink)', fontWeight: '600' } },
    status: {
      gridArea: 't', display: 'inline-flex', alignItems: 'center', gap: '3px', fontSize: '12px', lineHeight: '16px', color: 'var(--ink-muted)',
      '& .m-icon': { fontSize: '15px' }, '[data-pass=false] &': { color: 'var(--danger-ink)' },
    },
    swatches: { display: 'flex', flexWrap: 'wrap', gap: '6px' },
    swatchButton: {
      position: 'relative', width: '24px', height: '24px', padding: '0', border: '0', borderRadius: 'xs', cornerShape: 'var(--corner-shape-control)',
      backgroundColor: 'var(--pick)', cursor: 'pointer', boxShadow: 'inset 0 0 0 1px color-mix(in oklab, var(--ink) 22%, transparent)',
      '&:focus-visible': { outline: '2px solid var(--focus)', outlineOffset: '2px' },
      ...coarseHitArea,
    },
  } as any,
})

/* A value chosen inside a sentence, after Apple Shortcuts' parameters and human-readable filters:
   "While I'm away, GMs may move [Lady Maren and Anwen] along." The trigger reads as part of the
   sentence and opens the shared listbox; several values are summarised in words. */
export const inlineSelect = defineSlotRecipe({
  className: 'm-inline-select',
  description: 'A choice made inside a sentence; one value or several, summarised in words.',
  slots: ['root', 'trigger', 'value', 'indicator'],
  base: {
    root: { position: 'relative', display: 'inline' },
    trigger: {
      position: 'relative', display: 'inline', paddingInline: '4px 2px', paddingBlock: '1px 2px', marginInline: '-1px', border: '0',
      borderRadius: 'xs', cornerShape: 'var(--corner-shape-control)', boxDecorationBreak: 'clone', WebkitBoxDecorationBreak: 'clone',
      backgroundColor: 'color-mix(in oklab, var(--accent-soft) 55%, transparent)', color: 'var(--on-accent-soft)',
      font: 'inherit', fontWeight: '600', lineHeight: 'inherit', textAlign: 'start', cursor: 'pointer',
      textDecorationLine: 'underline', textDecorationStyle: 'dotted', textDecorationThickness: '1.5px', textUnderlineOffset: '0.24em',
      transition: motion.effects('background-color, color'),
      '@media (hover: hover)': { '&:hover:not(:disabled)': { backgroundColor: 'var(--accent-soft)' } },
      '&[aria-expanded=true]': { backgroundColor: 'var(--accent-soft)', textDecorationStyle: 'solid' },
      '&:focus-visible': { outline: '2px solid var(--focus)', outlineOffset: '1px' },
      '&:disabled': { cursor: 'not-allowed', color: 'var(--ink-muted)', backgroundColor: 'transparent' },
      '@media (forced-colors: active)': { border: '1px solid ButtonText', textDecorationStyle: 'solid' },
    },
    value: { overflowWrap: 'anywhere' },
    indicator: {
      fontSize: '1.05em', verticalAlign: '-0.18em', marginInlineStart: '1px', transition: motion.spatial('rotate'),
      '[aria-expanded=true] > &': { rotate: '180deg' },
    },
  } as any,
})
