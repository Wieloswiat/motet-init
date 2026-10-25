import { defineSlotRecipe } from '@pandacss/dev'
import { framed, motion } from '../tokens'

/* ---------------------------------------------------------------
   World scope. Art placement (data-art, set by the engine):
   - header:     art at the top of the page, scrolling away, ending in
                 the World's edge; the page is the World's surface.
   - continuous: art fixed behind the whole page. The top of the page
                 is a window onto it; the content sheet rises over it
                 with the World's edge and, on wide screens, dissolves
                 into it at the sides. One image, no frame.
   - backdrop:   the same, with only a band of art under the chrome.
   - none:       no art (none supplied, or decoration off).
   --------------------------------------------------------------- */
const FIXED_ART = ':is([data-art=continuous], [data-art=backdrop])'
const SHEET = 1080 /* content column, including its own padding */
/* Art treatments combine. Each variable is 0–1 and 0 is off: --art-veil lays page colour over the
   art, --art-vignette fades its edges into the page, --art-duotone recolours it in the accent and
   ornament hues, --art-blur softens it (up to 24px), --art-parallax lets it drift as the page
   scrolls (fixed art rises at up to half the page's speed; header art scrolls away at down to half). */
const artLayer = {
  opacity: 'calc(1 - var(--art-veil, 0) * 0.7)',
  filter: 'grayscale(var(--art-duotone, 0)) contrast(calc(1 + var(--art-duotone, 0) * 0.1)) blur(calc(var(--art-blur, 0) * 24px))',
  scale: 'calc(1 + var(--art-blur, 0) * 0.08)',
}
const duotoneLayer = {
  content: '""', position: 'absolute', inset: '0', pointerEvents: 'none',
  background: 'linear-gradient(150deg, var(--accent), var(--ornament))', mixBlendMode: 'color', opacity: 'calc(var(--art-duotone, 0) * 0.85)',
}
const vignette = (shape: string) => `radial-gradient(${shape}, #000 40%, rgb(0 0 0 / calc(1 - var(--art-vignette, 0))) 100%)`
/* Parallax moves the art layer and never resizes its box, so the image keeps its scale. Browsers
   with scroll-driven animations run it on the compositor: fixed art against the root scroller over
   the first screen, header art against its own box as it leaves the top of the viewport. Elsewhere
   the engine writes --art-drift (px scrolled through that same range) and the translate follows it.
   Readers who reduce motion never see it. */
const SCROLL_DRIVEN = '@supports (animation-timeline: view()) and (view-timeline-name: --m-art)'
const parallax = (dir: 1 | -1, keyframe: string, timeline: string, range: string, self: Record<string, string> = {}) => ({
  '[data-parallax=on] &::before': { translate: `0 calc(var(--art-drift, 0px) * var(--art-parallax, 0) * ${dir * 0.5})` },
  [SCROLL_DRIVEN]: {
    '[data-parallax=on] &': self,
    '[data-parallax=on] &::before': {
      animation: `m-fade-in var(--dur-decor) var(--ease-decor) both, ${keyframe} linear both`,
      animationTimeline: `auto, ${timeline}`,
      animationRange: `normal, ${range}`,
    },
  },
  '@media (prefers-reduced-motion: reduce)': { '[data-parallax=on] &::before': { animation: 'none', translate: 'none' } },
})

export const world = defineSlotRecipe({
  className: 'm-world',
  description: 'The scope a World paints. Everything inside reads the World’s variables.',
  slots: ['root', 'backdrop', 'page', 'layout', 'stream', 'rail', 'railLanes', 'sceneHeader', 'sceneTitle', 'sceneFields'],
  base: {
    root: {
      '--art-h': 'clamp(220px, 28vw, 360px)',
      /* vertical room the World's mark, title and byline need when they sit on the art */
      '--title-room': '196px',
      '@media (max-width: 640px)': { '--title-room': '236px' },
      '&:has(.m-masthead__root--size_slim)': {
        '--art-h': 'clamp(150px, 16vw, 210px)', '--title-room': '116px',
        '@media (max-width: 640px)': { '--title-room': '168px' },
      },
      position: 'relative', isolation: 'isolate', minHeight: '100vh', color: 'var(--ink)', fontFamily: 'ui',
      /* page-sized surfaces take grain only: a finish gradient stretched over a whole page reads as fog */
      backgroundColor: 'var(--surface)', backgroundImage: 'var(--grain)', paddingBottom: '48px',
      [`${FIXED_ART} &, &${FIXED_ART}`]: {
        backgroundColor: 'transparent', backgroundImage: 'none',
        /* the content sheet: surface and grain, masked by the World's edge at the top. A fade spans
           the full width; other edges make a column with hard sides. Text never sits in the mask. */
        '&::before': {
          content: '""', position: 'absolute', zIndex: '-1', insetInline: '0', bottom: '0', marginInline: 'auto',
          top: 'calc(var(--art-window, var(--art-h)) - var(--lip, 0px))',
          width: `var(--sheet-w, min(100%, ${SHEET}px))`,
          backgroundColor: 'var(--sheet-bg, var(--surface))', backgroundImage: 'var(--sheet-grain, var(--grain))', pointerEvents: 'none',
          backdropFilter: 'var(--sheet-filter, none)',
          boxShadow: 'inset 0 1px 0 var(--sheet-line, transparent), inset 1px 0 0 var(--sheet-line, transparent), inset -1px 0 0 var(--sheet-line, transparent)',
          maskImage: 'var(--sheet-edge), linear-gradient(#000 0 0)',
          maskSize: '100% var(--lip, 0px), 100% calc(100% - var(--lip, 0px) + 1px)',
          maskPosition: '0 0, 0 calc(var(--lip, 0px) - 1px)',
          maskRepeat: 'no-repeat',
          [`@media (max-width: ${SHEET}px)`]: { boxShadow: 'inset 0 1px 0 var(--sheet-line, transparent)' },
        },
      },
    },
    backdrop: {
      display: 'none', position: 'fixed', inset: '0', zIndex: '-2', overflow: 'hidden', backgroundColor: 'var(--surface)', pointerEvents: 'none',
      [`${FIXED_ART} &`]: { display: 'block' },
      /* continuous: the art's focus point sits in the header window at load, at 60% of its height.
         Wide screens size the art to the viewport and let its lower edge fade into the surface;
         narrow screens (no margins to fill) size it to the width, as header art would be. */
      '--fy': 'var(--art-fy, 0.5)',
      '--win': 'var(--art-window, var(--art-h))',
      '--box': 'max(100vh, 56.25vw)',
      [`@media (max-width: ${SHEET + 111}px)`]: { '--box': 'max(56.25vw, var(--win))' },
      '&::before': {
        content: '""', position: 'absolute', inset: '0', backgroundImage: 'var(--art, none)', backgroundSize: 'cover',
        backgroundPosition: 'var(--art-focus, 50% 50%)',
        animation: 'm-fade-in var(--dur-decor) var(--ease-decor) both',
        ...artLayer,
        maskImage: vignette('130% 100% at 50% 20%'),
      },
      '[data-art=continuous] &::before': {
        inset: 'auto 0', top: 'min(0px, calc(var(--win) * 0.6 - var(--fy) * var(--box)))', height: 'var(--box)',
        backgroundPosition: '50% calc(var(--fy) * 100%)',
        maskImage: `linear-gradient(to bottom, #000 70%, transparent), ${vignette('130% 100% at 50% 20%')}`,
        maskComposite: 'intersect',
      },
      /* rising backdrop art fades out at its foot, so the surface it uncovers never meets a hard edge */
      '[data-art=backdrop][data-parallax=on] &::before': {
        maskImage: `linear-gradient(to bottom, #000 55%, transparent), ${vignette('130% 100% at 50% 20%')}`, maskComposite: 'intersect',
      },
      '&::after': duotoneLayer,
      ...parallax(-1, 'm-drift-up', 'scroll(root)', '0 100vh'),
    },
    /* the page and each layout column are inline-size containers, so rows and cards inside
       (scene rows, character cards) adapt to the column they sit in, not to the window */
    page: {
      position: 'relative', maxWidth: `${SHEET}px`, marginInline: 'auto', paddingInline: '32px', containerType: 'inline-size',
      '@media (max-width: 640px)': { paddingInline: '16px' },
    },
    layout: {
      display: 'grid', gridTemplateColumns: 'minmax(0, 1fr)', gap: '0 40px',
      '& > *': { containerType: 'inline-size' },
      '@media (min-width: 1060px)': { gridTemplateColumns: 'minmax(0, 1fr) 256px' },
    },
    stream: {
      containerType: 'inline-size', containerName: 'stream', minWidth: '0',
      /* the lane filter hides a lane's posts and the dividers that only separated them */
      '&[data-show=ic] .m-post__root--lane_ooc, &[data-show=ic] .m-post__root--lane_ooc + .m-divider__root, &[data-show=ic] .m-divider__root:has(+ .m-post__root--lane_ooc)': { display: 'none' },
      '&[data-show=ooc] .m-post__root--lane_ic, &[data-show=ooc] .m-post__root--lane_ic + .m-divider__root, &[data-show=ooc] .m-divider__root:has(+ .m-post__root--lane_ic)': { display: 'none' },
      '&[data-show=ic] .m-event__root[data-lane=ooc], &[data-show=ooc] .m-event__root:not([data-lane=ooc])': { display: 'none' },
    },
    rail: {
      display: 'none',
      '@media (min-width: 1060px)': { display: 'block', position: 'sticky', top: 'calc(var(--chrome-offset, 72px) + 8px)', alignSelf: 'start', marginTop: '8px' },
    },
    railLanes: {
      display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '12px', marginBottom: '12px',
      fontSize: '13px', lineHeight: '18px', fontWeight: '600', color: 'var(--ink-muted)',
    },
    sceneHeader: { paddingBlock: '28px 12px' },
    sceneTitle: {
      margin: '0', fontFamily: 'display', fontWeight: 'var(--display-weight)', textTransform: 'var(--display-case)',
      letterSpacing: 'var(--display-tracking)', fontVariationSettings: 'var(--display-axes)',
      fontSize: 'calc(30px * var(--display-adjust))', lineHeight: '1.1', textWrap: 'balance', color: 'var(--ink)',
      /* left-aligned headings take only the leading flank, inline, so it stays with the first line when the title wraps */
      '&.m-flanked': { display: 'block' },
      '&.m-flanked::before': { verticalAlign: '0.3em', marginInlineEnd: '14px' },
      '&.m-flanked::after': { display: 'none' },
    },
    sceneFields: {
      display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '8px 20px', marginTop: '12px',
      fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)',
      '& .m-segmented__root, & .m-lanes__root': { marginInlineStart: 'auto' },
      /* on wide screens the lane filter lives at the top of the sticky rail instead */
      '@media (min-width: 1060px)': { '& .m-lanes__root': { display: 'none' } },
    },
  } as any,
})

/* ---------------------------------------------------------------
   Post: character voice first, account attribution always present.
   Layouts: open (dividers), card (framed), dossier (a filed document
   with a form header) and log (ruled rows, speaker in the accent).
   --------------------------------------------------------------- */
export const post = defineSlotRecipe({
  className: 'm-post',
  slots: ['root', 'portrait', 'main', 'header', 'mini', 'speaker', 'name', 'epithet', 'author', 'handle', 'meta', 'follows', 'body', 'actions'],
  base: {
    root: {
      '--portrait-col': '56px', position: 'relative', display: 'grid', gridTemplateColumns: 'var(--portrait-col) minmax(0, 1fr)',
      columnGap: '20px', paddingBlock: 'calc(var(--post-gap) / 2)',
      '[data-portrait=inline] &, [data-portrait=none] &': { gridTemplateColumns: 'minmax(0, 1fr)' },
      '[data-post=card] &': {
        ...framed('var(--surface-raised)'), paddingInline: '24px', paddingTop: '20px', paddingBottom: '12px', marginBlock: 'calc(var(--post-gap) / 2)',
        boxShadow: 'var(--elevation-1), var(--edge-light)',
      },
      '[data-post=card][data-frame=double] &': { outline: '1px solid var(--line)', outlineOffset: '-6px' },
      '[data-post=dossier] &': {
        ...framed('var(--surface-raised)'), padding: '0', overflow: 'clip', marginBlock: 'calc(var(--post-gap) / 2)', boxShadow: 'var(--elevation-1)',
        gridTemplateColumns: 'minmax(0, 1fr)',
      },
      '[data-post=log] &': { paddingBlock: '16px', boxShadow: 'inset 0 -1px 0 var(--line)' },
      '@container stream (max-width: 600px)': { gridTemplateColumns: 'minmax(0, 1fr)' },
    },
    portrait: {
      position: 'sticky', top: 'calc(var(--chrome-offset, 72px) + 8px)', alignSelf: 'start', paddingTop: '2px',
      '[data-portrait=inline] &, [data-portrait=none] &, [data-post=dossier] &': { display: 'none' },
      '@container stream (max-width: 600px)': { display: 'none' },
    },
    main: { minWidth: '0' },
    header: {
      display: 'grid', gridTemplateColumns: 'auto minmax(0, 1fr) auto', gridTemplateAreas: '"mini speaker meta" "mini author meta"',
      alignItems: 'center', columnGap: '12px', rowGap: '2px',
      '[data-post=dossier] &': { paddingInline: '16px 12px', paddingBlock: '10px', backgroundColor: 'var(--surface-quiet)', boxShadow: 'inset 0 -1px 0 var(--line)' },
      '[data-portrait=none] &': { gridTemplateColumns: 'minmax(0, 1fr) auto', gridTemplateAreas: '"speaker meta" "author meta"' },
      /* narrow streams: the account line runs under the time, so handles keep their width */
      '@container stream (max-width: 600px)': { gridTemplateAreas: '"mini speaker meta" "mini author author"' },
      '[data-portrait=none] &:is(*)': { '@container stream (max-width: 600px)': { gridTemplateAreas: '"speaker meta" "author author"' } },
    },
    mini: {
      gridArea: 'mini', display: 'none',
      '[data-portrait=inline] &, [data-post=dossier] &': { display: 'block' },
      '[data-portrait=none] &': { display: 'none !important' },
      '@container stream (max-width: 600px)': { display: 'block' },
    },
    speaker: { gridArea: 'speaker', display: 'flex', flexWrap: 'wrap', alignItems: 'baseline', columnGap: '10px', minWidth: '0' },
    name: {
      fontFamily: 'name', fontSize: 'calc(17px * var(--name-adjust, 1))', lineHeight: '1.25', fontWeight: '600',
      textTransform: 'var(--name-case, none)', letterSpacing: 'var(--name-tracking, 0)', fontVariationSettings: 'var(--name-axes, normal)',
      color: 'var(--ink)', overflowWrap: 'anywhere',
      '[data-names=plate] &': { width: 'fit-content', backgroundColor: 'var(--plate-bg)', color: 'var(--plate-ink)', paddingInline: '8px', paddingBlock: '1px 0', borderRadius: 'var(--radius-xs)' },
      '[data-post=log] &': { color: 'var(--accent-ink)' },
    },
    epithet: { fontFamily: 'ui', fontSize: '13.5px', lineHeight: '18px', color: 'var(--ink-muted)' },
    author: {
      gridArea: 'author', display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '4px 8px', minWidth: '0',
      fontFamily: 'ui', fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)',
      '& > .m-icon': { fontSize: '16px', marginInlineEnd: '-4px' },
    },
    handle: { overflowWrap: 'anywhere' },
    meta: {
      gridArea: 'meta', alignSelf: 'start', display: 'flex', alignItems: 'center', gap: '2px', marginTop: '-4px',
      fontFamily: 'ui', fontSize: '13px', color: 'var(--ink-muted)', fontVariantNumeric: 'tabular-nums', whiteSpace: 'nowrap',
      '& time': { paddingInline: '6px' },
      '& > .m-icon': { fontSize: '16px' },
      '[data-post=dossier] &': { marginTop: '0', alignSelf: 'center' },
    },
    /* a link back to the Scene event this post answers (a roll, a tool's result); events never sit inside posts */
    follows: {
      display: 'flex', alignItems: 'center', gap: '6px', width: 'fit-content', maxWidth: '100%', marginTop: '12px',
      fontFamily: 'ui', fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)', textDecoration: 'none', borderRadius: 'sm',
      '& .m-icon': { fontSize: '16px', flexShrink: 0 },
      '& b': { fontWeight: 600, color: 'var(--ink)' },
      '&:hover': { textDecoration: 'underline', textUnderlineOffset: '3px' },
      '&:focus-visible': { outline: '2px solid var(--focus)', outlineOffset: '2px' },
      '& + .m-post__body': { marginTop: '6px' },
      '[data-post=dossier] &': { marginInline: '20px' },
    },
    body: {
      marginTop: '14px',
      '[data-post=dossier] &': { paddingInline: '20px', marginTop: '16px' },
    },
    actions: {
      display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '2px 4px', marginTop: '10px', marginInlineStart: '-12px',
      '[data-post=dossier] &': { marginInlineStart: '8px', paddingBottom: '8px' },
    },
  } as any,
  variants: {
    lane: {
      ic: {},
      ooc: {
        root: {
          '--portrait-col': '40px', backgroundColor: 'var(--surface-quiet) !important', backgroundImage: 'none !important', borderRadius: 'var(--radius-md) !important',
          cornerShape: 'var(--corner-shape) !important', paddingInline: '16px !important', paddingTop: '14px !important', paddingBottom: '8px !important', marginBlock: '8px',
          border: '0 !important', boxShadow: 'none !important', outline: '0 !important', gridTemplateColumns: 'var(--portrait-col) minmax(0, 1fr)',
        },
        portrait: { display: 'block !important' },
        mini: { display: 'none !important' },
        header: { backgroundColor: 'transparent !important', padding: '0 !important', boxShadow: 'none !important' },
        name: { fontFamily: 'ui', fontSize: '15px', textTransform: 'none', letterSpacing: '0', fontVariationSettings: 'normal', backgroundColor: 'transparent !important', color: 'var(--ink) !important', padding: '0 !important' },
        body: { marginTop: '8px', paddingInline: '0 !important' },
        actions: { marginInlineStart: '-12px !important', paddingBottom: '0 !important' },
      },
    },
  } as any,
  staticCss: [{ lane: ['*'] }],
})

/* ---------------------------------------------------------------
   Divider: a hairline in Motet; a fading rule, the filigree motif or
   the offset accent segment when a World chooses one
   --------------------------------------------------------------- */
export const divider = defineSlotRecipe({
  className: 'm-divider',
  slots: ['root', 'line', 'motif'],
  base: {
    root: {
      display: 'flex', alignItems: 'center', gap: 'calc(14px * min(var(--orn-level, 0), 1))', color: 'var(--divider-color, var(--line))',
      marginBlock: 'calc(var(--post-gap) * 0.6)',
      '[data-post=card] .m-world__stream > &, [data-post=dossier] .m-world__stream > &, [data-post=log] .m-world__stream > &': { display: 'none' },
      '[data-orn=segment] &, [data-orn=fade] &, [data-orn=rule] &': { gap: '0' },
    },
    line: {
      flex: '1', height: '1px', backgroundColor: 'currentColor',
      '[data-orn=segment] &': { backgroundColor: 'var(--line-strong)' },
      '[data-orn=segment] &:first-child': { display: 'none' },
      '[data-orn-line=fade] &:first-child': { backgroundColor: 'transparent', backgroundImage: 'linear-gradient(to right, transparent, currentColor)' },
      '[data-orn-line=fade] &:last-child': { backgroundColor: 'transparent', backgroundImage: 'linear-gradient(to left, transparent, currentColor)' },
    },
    motif: {
      display: 'var(--orn-motif-display, none)', flex: 'none', width: 'var(--orn-motif-w, 0px)', height: 'var(--orn-motif-h, 0px)',
      backgroundColor: 'currentColor', maskImage: 'var(--orn-motif, none)', maskSize: 'contain', maskRepeat: 'no-repeat', maskPosition: 'center',
      animation: 'm-fade-in var(--dur-decor) var(--ease-decor) both',
      '[data-orn=segment] &': { marginInlineEnd: '6px' },
    },
  } as any,
})

/* ---------------------------------------------------------------
   Masthead: full-bleed header art that fades or breaks into the page;
   identity and tabs sit below it on the surface
   --------------------------------------------------------------- */
export const masthead = defineSlotRecipe({
  className: 'm-masthead',
  slots: ['root', 'art', 'scrim', 'inner', 'identity', 'mark', 'titles', 'title', 'byline', 'nav'],
  base: {
    root: { position: 'relative', '--art-h': 'clamp(220px, 28vw, 360px)' },
    art: {
      /* header placement draws the art here; continuous and backdrop leave a clear window onto the
         World's fixed art, so the header and the background are one image */
      position: 'relative', height: 'var(--art-window, var(--art-h))', overflow: 'hidden', mask: 'var(--art-mask)',
      '&::before': {
        content: '""', position: 'absolute', inset: '0', backgroundImage: 'var(--art-masthead, var(--art, none))', backgroundSize: 'cover',
        backgroundPosition: 'var(--art-focus, 50% 50%)',
        animation: 'm-fade-in var(--dur-decor) var(--ease-decor) both',
        ...artLayer,
        maskImage: vignette('140% 110% at 50% 10%'),
      },
      /* the art's own box is the parallax timeline: the layer sinks while the box leaves the top of the
         viewport, so the strip it uncovers is always above the screen */
      '&::after': duotoneLayer,
      ...parallax(1, 'm-drift-down', '--m-art', 'exit-crossing 0% exit-crossing 100%', { viewTimelineName: '--m-art', viewTimelineAxis: 'block', viewTimelineInset: '0px' /* not the page's scroll padding */ }),
      [`${FIXED_ART} &, [data-art=none] &`]: { mask: 'none', '&::before, &::after': { display: 'none' } },
    },
    /* title on art: a scrim under the World's mark and title, as opaque as the engine measured the
       art needs (--title-scrim). Fixed art keeps it under the content sheet so the World's edge stays clean. */
    scrim: {
      display: 'none', position: 'absolute', inset: '0', pointerEvents: 'none',
      background: 'linear-gradient(to bottom, transparent calc(100% - var(--title-gap, 20px) - var(--title-room, 200px) - 24px), var(--title-scrim, transparent) calc(100% - var(--title-gap, 20px) - var(--title-room, 200px) * 0.45), var(--title-scrim, transparent))',
      '[data-title=art] &': { display: 'block' },
      [`${FIXED_ART} &`]: { zIndex: '-2' },
    },
    inner: {
      position: 'relative', maxWidth: `${SHEET}px`, marginInline: 'auto', paddingInline: '32px', '@media (max-width: 640px)': { paddingInline: '16px' },
      /* clear space between the World's edge and its title */
      ':is([data-art=continuous], [data-art=backdrop], [data-art=header])[data-title=page] &': { paddingTop: '22px' },
      '[data-title=art] &': { paddingTop: '8px' },
      '@media (min-width: 641px)': { '[data-title=hidden] &': { paddingTop: '16px' } },
    },
    identity: {
      position: 'relative', display: 'grid', gridTemplateColumns: 'auto minmax(0, 1fr)', alignItems: 'end', columnGap: '20px', rowGap: '10px',
      '@media (max-width: 640px)': { gridTemplateColumns: 'minmax(0, 1fr)', alignItems: 'start' },
      /* title on art: the identity stands on the art, its bottom a gap above the World's edge */
      '[data-title=art] &': {
        position: 'absolute', insetInline: '32px', top: 'calc(-8px - var(--title-gap, 20px))', translate: '0 -100%',
        '--ink': 'var(--title-ink)', '--ink-muted': 'var(--title-ink)', textShadow: 'var(--title-halo, none)',
        '@media (max-width: 640px)': { insetInline: '16px' },
      },
    },
    mark: {
      /* title in the image: the World's art carries its own wordmark. On phone widths the art is too
         small for that wordmark to read, so the mark and title stay */
      '@media (min-width: 641px)': { '[data-title=hidden] &': { display: 'none' } },
      '--avatar-size': '96px', marginTop: 'calc(var(--mark-lift, 64px) * -1)', boxShadow: '0 0 0 4px var(--surface), var(--elevation-2)',
      '@media (max-width: 640px)': { '--avatar-size': '64px', marginTop: 'calc(var(--mark-lift, 40px) * -1)' },
    },
    titles: { minWidth: '0', paddingBottom: '2px' },
    title: {
      margin: '0', fontFamily: 'display', fontWeight: 'var(--display-weight)', textTransform: 'var(--display-case)',
      letterSpacing: 'var(--display-tracking)', fontVariationSettings: 'var(--display-axes)', color: 'var(--ink)',
      fontSize: 'calc(clamp(28px, 4vw, 44px) * var(--display-adjust))', lineHeight: '1.05', textWrap: 'balance',
      '@media (min-width: 641px)': {
        '[data-title=hidden] &': {
          position: 'absolute !important', width: '1px !important', height: '1px !important', overflow: 'hidden', clipPath: 'inset(50%)', whiteSpace: 'nowrap', margin: '-1px',
        },
      },
    },
    byline: {
      display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '8px 18px', marginTop: '10px',
      fontFamily: 'ui', fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)',
      '@media (min-width: 641px)': { '[data-title=hidden] &': { marginTop: '0' } },
    },
    nav: { marginTop: '14px', marginInline: '-12px' },
  } as any,
  variants: {
    size: {
      full: {},
      slim: {
        root: { '--art-h': 'clamp(150px, 16vw, 210px)' },
        mark: { '--avatar-size': '64px', marginTop: 'calc(var(--mark-lift, 40px) * -1)' },
        title: { fontSize: 'calc(26px * var(--display-adjust))' },
        byline: { marginTop: '6px' },
      },
    },
  } as any,
  staticCss: [{ size: ['*'] }],
})

/* ---------------------------------------------------------------
   Chrome: Motet's own corner islands. They reset every World variable
   they touch, so a World can tint them but never restyle them.
   --------------------------------------------------------------- */
export const chrome = defineSlotRecipe({
  className: 'm-chrome',
  slots: ['root', 'island', 'home', 'world', 'search', 'kbd', 'sep'],
  base: {
    root: {
      position: 'fixed', top: '12px', insetInline: '12px', zIndex: 'chrome', display: 'flex', justifyContent: 'space-between',
      alignItems: 'flex-start', gap: '12px', pointerEvents: 'none',
      '@media (max-width: 480px)': { top: '8px', insetInline: '8px' },
    },
    island: {
      '--ink': 'var(--chrome-ink)', '--ink-muted': 'var(--chrome-ink-muted)', '--accent': 'var(--chrome-accent)',
      '--on-accent': 'var(--chrome-on-accent)', '--accent-soft': 'var(--chrome-accent-soft)', '--on-accent-soft': 'var(--chrome-on-accent-soft)',
      '--surface-quiet': 'var(--chrome-hover)', '--surface-raised': 'var(--chrome-bg-solid)', '--surface-overlay': 'var(--chrome-bg-solid)',
      '--line-strong': 'var(--chrome-ink-muted)', '--focus': 'var(--chrome-accent)', '--corner-shape': 'squircle',
      '--radius-xs': '4px', '--radius-sm': '8px', '--radius-md': '12px', '--radius-lg': '18px', '--edge-light': '0 0 #0000',
      '--radius-control': '12px', '--corner-shape-control': 'squircle', '--control-h': '40px', '--count-ring': 'var(--chrome-bg-solid)',
      '--icon-family': '"Material Symbols Outlined"', '--icon-wght': '400', '--line': 'var(--chrome-line)',
      '--menu-bg': 'var(--chrome-bg-solid)', '--menu-ink': 'var(--chrome-ink)', '--menu-ink-muted': 'var(--chrome-ink-muted)',
      '--menu-line': 'var(--chrome-line)', '--menu-accent': 'var(--chrome-accent)',
      pointerEvents: 'auto', display: 'flex', alignItems: 'center', gap: '2px', padding: '4px', minHeight: '50px',
      backgroundColor: 'var(--chrome-bg)', backgroundImage: 'var(--chrome-grain)',
      backdropFilter: 'blur(var(--chrome-blur)) saturate(var(--chrome-saturate))',
      border: '1px solid var(--chrome-line)', borderRadius: 'chrome', cornerShape: 'squircle', color: 'var(--chrome-ink)',
      boxShadow: 'var(--chrome-shadow)', fontFamily: 'ui', fontSize: '14px',
      transition: motion.effects('background-color'),
      '@supports (corner-shape: squircle)': { borderRadius: '24px' },
      '@media (prefers-reduced-transparency: reduce)': { backgroundColor: 'var(--chrome-bg-solid)', backdropFilter: 'none', backgroundImage: 'none' },
      '[data-reduce-transparency] &': { backgroundColor: 'var(--chrome-bg-solid)', backdropFilter: 'none', backgroundImage: 'none' },
    },
    home: {
      position: 'relative', display: 'inline-flex', alignItems: 'center', gap: '8px', height: '40px', paddingInline: '10px 12px',
      border: '0', borderRadius: '12px', cornerShape: 'squircle', backgroundColor: 'transparent', color: 'var(--chrome-ink)',
      font: '600 16px/1 var(--font-ui)', letterSpacing: '-0.02em', textDecoration: 'none', cursor: 'pointer',
      backgroundImage: 'linear-gradient(var(--state, transparent), var(--state, transparent))',
      '@media (hover: hover)': { '&:hover': { '--state': 'var(--chrome-hover)' } },
    },
    world: {
      position: 'relative', display: 'inline-flex', alignItems: 'center', gap: '2px', height: '40px', paddingInline: '6px 4px',
      border: '0', borderRadius: '12px', cornerShape: 'squircle', backgroundColor: 'transparent', color: 'var(--chrome-ink-muted)', cursor: 'pointer',
      backgroundImage: 'linear-gradient(var(--state, transparent), var(--state, transparent))',
      '@media (hover: hover)': { '&:hover': { '--state': 'var(--chrome-hover)' } },
      '&[aria-expanded=true]': { '--state': 'var(--chrome-hover)' },
      '& .m-icon': { fontSize: '18px' },
    },
    search: {
      position: 'relative', display: 'inline-flex', alignItems: 'center', gap: '8px', height: '40px', width: '220px', paddingInline: '10px 6px',
      border: '0', borderRadius: '12px', cornerShape: 'squircle', backgroundColor: 'var(--chrome-hover)', color: 'var(--chrome-ink-muted)',
      font: '400 14px/20px var(--font-ui)', cursor: 'text', textAlign: 'start',
      '& .m-chrome__label': { flex: '1' },
      '@media (max-width: 760px)': { width: '40px', paddingInline: '0', justifyContent: 'center', backgroundColor: 'transparent', '& > :not(.m-icon)': { display: 'none' } },
    },
    kbd: {
      display: 'inline-grid', placeItems: 'center', minWidth: '22px', height: '22px', paddingInline: '5px', borderRadius: '6px',
      border: '1px solid var(--chrome-line)', font: '500 12px/1 var(--font-mono)', color: 'var(--chrome-ink-muted)',
    },
    sep: { width: '1px', height: '24px', marginInline: '2px', backgroundColor: 'var(--chrome-line)' },
  } as any,
})

/* ---------------------------------------------------------------
   Composer: the writing plane dominates; speaker, destination and
   audience in one row above it, tools and Post below
   --------------------------------------------------------------- */
export const composer = defineSlotRecipe({
  className: 'm-composer',
  slots: ['root', 'head', 'speaker', 'speakerName', 'destination', 'audience', 'editor', 'foot', 'tools', 'status', 'submit'],
  base: {
    root: {
      ...framed('var(--surface-raised)'), position: 'relative', display: 'grid', gridTemplateRows: 'auto minmax(0, 1fr) auto',
      boxShadow: 'var(--elevation-3), var(--edge-light)',
      containerType: 'inline-size', containerName: 'composer',
      transition: motion.effects('box-shadow'),
      '&:focus-within': { boxShadow: '0 0 0 2px var(--focus), var(--elevation-3)' },
      '&[data-expanded]': { position: 'fixed', inset: 'calc(var(--chrome-offset, 72px)) 16px 16px', zIndex: 'overlay', maxWidth: '920px', marginInline: 'auto' },
    },
    head: {
      display: 'flex', alignItems: 'center', gap: '8px', minWidth: '0', paddingInline: '8px 6px', paddingBlock: '6px',
      boxShadow: 'inset 0 -1px 0 var(--line)',
    },
    speaker: {
      position: 'relative', flex: 'none', display: 'inline-flex', alignItems: 'center', gap: '10px', minHeight: '44px', paddingInline: '4px 8px',
      border: '0', borderRadius: 'md', cornerShape: 'var(--corner-shape-control)', backgroundColor: 'transparent', color: 'var(--ink)', cursor: 'pointer',
      backgroundImage: 'linear-gradient(var(--state, transparent), var(--state, transparent))',
      '@media (hover: hover)': { '&:hover': { '--state': 'color-mix(in oklab, currentColor 7%, transparent)' } },
      '& > .m-icon': { color: 'var(--ink-muted)', fontSize: '18px' },
    },
    speakerName: {
      fontFamily: 'name', fontSize: 'calc(15px * var(--name-adjust, 1))', fontWeight: '600', lineHeight: '20px',
      textTransform: 'var(--name-case, none)', letterSpacing: 'var(--name-tracking, 0)', fontVariationSettings: 'var(--name-axes, normal)',
    },
    destination: {
      flex: '0 1 auto', display: 'inline-flex', alignItems: 'center', gap: '6px', minWidth: '0', fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)',
      '& .m-icon': { fontSize: '16px', flex: 'none' },
      '& strong': { fontWeight: '500', color: 'var(--ink)', minWidth: '0', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' },
      '& .m-chip': { flex: 'none' },
      /* narrow: the lane chip goes first, then the scene name truncates; speaker and audience stay whole */
      '@container composer (max-width: 600px)': { '& .m-chip': { display: 'none' } },
      '@container composer (max-width: 400px)': { display: 'none' },
    },
    audience: { marginInlineStart: 'auto', flex: 'none', display: 'flex', alignItems: 'center', gap: '2px' },
    editor: {
      display: 'block', width: '100%', minHeight: '168px', paddingInline: '20px', paddingBlock: '16px', border: '0', resize: 'none',
      backgroundColor: 'transparent', color: 'var(--ink)', fontFamily: 'reading', fontSize: 'calc(17px * var(--text-scale, 1))', lineHeight: '1.6',
      '&:focus': { outline: 'none' },
      '&::placeholder': { color: 'var(--ink-muted)' },
      '[data-expanded] &': { height: '100%' },
    },
    foot: {
      display: 'flex', alignItems: 'center', gap: '8px', paddingInline: '6px 8px', paddingBlock: '6px 8px', boxShadow: 'inset 0 1px 0 var(--line)',
    },
    tools: { display: 'flex', alignItems: 'center', gap: '0', '@container composer (max-width: 520px)': { '& [data-optional]': { display: 'none' } } },
    submit: { marginInlineStart: 'auto', display: 'flex', alignItems: 'center', gap: '6px' },
    status: {
      display: 'inline-grid', placeItems: 'center', width: '32px', height: '32px', borderRadius: '8px', color: 'var(--ink-muted)', cursor: 'default',
      '& .m-icon': { fontSize: '20px' },
      '&[data-tone=caution]': { color: 'var(--caution-ink)' },
      '&[data-tone=danger]': { color: 'var(--danger-ink)' },
    },
  } as any,
})

/* ---------------------------------------------------------------
   Dice: the result inside a Scene event (see `event`). A roll is never part of a post.
   Faces come from DiceFont.
   --------------------------------------------------------------- */
export const roll = defineSlotRecipe({
  className: 'm-roll',
  slots: ['root', 'label', 'what', 'expr', 'dice', 'die', 'total', 'outcome'],
  base: {
    root: {
      display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) auto', gridTemplateAreas: '"label total" "dice total" "outcome outcome"',
      alignItems: 'center', gap: '6px 16px', maxWidth: '68ch', marginTop: '14px', marginInline: '0', padding: '12px 16px',
      backgroundColor: 'var(--surface-quiet)', border: '1px solid var(--line)', borderRadius: 'md', cornerShape: 'var(--corner-shape)',
      fontFamily: 'ui',
      '[data-post=dossier] &': { marginInline: '20px' },
      /* inside a scene event the event card is the frame */
      '.m-event__body > &, [data-post=dossier] .m-event__body > &': { margin: '0', padding: '0', border: '0', borderRadius: '0', backgroundColor: 'transparent', maxWidth: 'none' },
    },
    label: { gridArea: 'label', display: 'flex', flexWrap: 'wrap', alignItems: 'baseline', gap: '2px 10px' },
    what: { fontSize: '14px', fontWeight: '600', lineHeight: '20px', color: 'var(--ink)' },
    expr: { fontFamily: 'mono', fontSize: '13px', color: 'var(--ink-muted)' },
    dice: { gridArea: 'dice', display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '6px' },
    die: {
      position: 'relative', display: 'grid', placeItems: 'center', minWidth: '28px', height: '28px', paddingInline: '4px', borderRadius: 'xs', cornerShape: 'var(--corner-shape-control)',
      backgroundColor: 'var(--surface-raised)', boxShadow: 'inset 0 0 0 1px var(--line-strong)', fontSize: '14px', fontWeight: '600', fontVariantNumeric: 'tabular-nums',
      '&.m-roll__die--glyph': { minWidth: '0', height: 'auto', padding: '0', backgroundColor: 'transparent', boxShadow: 'none', color: 'var(--ink)' },
      '& .m-die': { fontFamily: 'var(--font-dice)', fontSize: '44px', lineHeight: '1', fontWeight: 'normal', display: 'block' },
      '&[data-dropped]': { color: 'var(--ink-muted)', boxShadow: 'inset 0 0 0 1px var(--line)', backgroundColor: 'transparent' },
      '&[data-dropped]::after': { content: '""', position: 'absolute', left: '-2px', right: '-2px', top: '50%', height: '2px', backgroundColor: 'var(--ink-muted)', rotate: '-35deg' },
      '&.m-roll__die--glyph[data-dropped]': { boxShadow: 'none', '& .m-die': { opacity: '0.5' } },
    },
    total: {
      gridArea: 'total', alignSelf: 'stretch', display: 'grid', placeItems: 'center', minWidth: '56px', paddingInlineStart: '16px',
      boxShadow: 'inset 1px 0 0 var(--line)', fontSize: '28px', lineHeight: '32px', fontWeight: '700', fontVariantNumeric: 'tabular-nums', color: 'var(--ink)',
    },
    outcome: {
      gridArea: 'outcome', display: 'flex', alignItems: 'center', gap: '6px', marginTop: '2px', fontSize: '13px', color: 'var(--ink)',
      '& .m-icon': { fontSize: '16px', color: 'var(--positive-ink)' },
      '&[data-tone=danger] .m-icon': { color: 'var(--danger-ink)' }, '&[data-tone=caution] .m-icon': { color: 'var(--caution-ink)' },
    },
  } as any,
})

/* ---------------------------------------------------------------
   Scene list and inbox: unread, requested response and draft are
   three separate signals. The cast sits in a fixed column just
   before the time, so rows line up, and is the first thing to go
   when the list narrows.
   --------------------------------------------------------------- */
export const sceneRow = defineSlotRecipe({
  className: 'm-scene-row',
  slots: ['root', 'marker', 'main', 'title', 'where', 'cast', 'more', 'state', 'time'],
  base: {
    root: {
      position: 'relative', display: 'grid', gridTemplateColumns: '3px minmax(0, 1fr) auto 84px 5ch', alignItems: 'center', columnGap: '14px',
      minHeight: '64px', paddingBlock: '10px', paddingInlineEnd: '14px', color: 'var(--ink)', textDecoration: 'none',
      boxShadow: 'inset 0 -1px 0 var(--line)', fontFamily: 'ui',
      backgroundImage: 'linear-gradient(var(--state, transparent), var(--state, transparent))', transition: motion.effects('background-color'),
      '@media (hover: hover)': { '&:hover': { '--state': 'color-mix(in oklab, var(--ink) 4%, transparent)' } },
      '@container (max-width: 640px)': { gridTemplateColumns: '3px minmax(0, 1fr) auto 5ch' },
      '@container (max-width: 460px)': { gridTemplateColumns: '3px minmax(0, 1fr) 5ch', gridTemplateAreas: '"marker main time" "marker state state"', rowGap: '6px' },
    },
    marker: {
      alignSelf: 'stretch', marginBlock: '6px', borderRadius: '0 3px 3px 0', backgroundColor: 'transparent',
      '@container (max-width: 460px)': { gridArea: 'marker' },
    },
    main: { minWidth: '0', '@container (max-width: 460px)': { gridArea: 'main' } },
    title: { display: 'block', fontSize: '15px', lineHeight: '20px', fontWeight: '500', overflowWrap: 'anywhere' },
    where: { display: 'flex', flexWrap: 'wrap', gap: '2px 14px', marginTop: '3px', fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)' },
    state: { display: 'flex', alignItems: 'center', justifyContent: 'flex-end', gap: '6px', '@container (max-width: 460px)': { gridArea: 'state', justifyContent: 'flex-start' } },
    cast: {
      display: 'flex', alignItems: 'center', justifyContent: 'flex-end', '& > .m-avatar': { boxShadow: '0 0 0 2px var(--surface)' }, '& > .m-avatar + .m-avatar': { marginInlineStart: '-6px' },
      '@container (max-width: 640px)': { display: 'none' },
    },
    more: { marginInlineEnd: '6px', fontSize: '12.5px', color: 'var(--ink-muted)', fontVariantNumeric: 'tabular-nums' },
    time: { fontSize: '13px', color: 'var(--ink-muted)', textAlign: 'end', fontVariantNumeric: 'tabular-nums', whiteSpace: 'nowrap', '@container (max-width: 460px)': { gridArea: 'time', alignSelf: 'start' } },
  } as any,
  variants: {
    unread: {
      true: { marker: { backgroundColor: 'var(--accent)' }, title: { fontWeight: '650' } },
      false: {},
    },
  } as any,
  staticCss: [{ unread: ['*'] }],
})

export const inboxItem = defineSlotRecipe({
  className: 'm-inbox-item',
  slots: ['root', 'world', 'main', 'kind', 'in', 'title', 'excerpt', 'side'],
  base: {
    root: {
      position: 'relative', display: 'grid', gridTemplateColumns: '28px minmax(0, 1fr) auto', columnGap: '14px', alignItems: 'start',
      paddingBlock: '12px', paddingInline: '14px', color: 'var(--ink)', textDecoration: 'none', fontFamily: 'ui', boxShadow: 'inset 0 -1px 0 var(--line)',
      backgroundImage: 'linear-gradient(var(--state, transparent), var(--state, transparent))',
      '@media (hover: hover)': { '&:hover': { '--state': 'color-mix(in oklab, var(--ink) 4%, transparent)' } },
      '&::before': { content: '""', position: 'absolute', insetBlock: '12px', insetInlineStart: '0', width: '3px', borderRadius: '0 3px 3px 0', backgroundColor: 'transparent' },
    },
    world: { paddingTop: '2px' },
    main: { display: 'grid', gap: '2px', minWidth: '0' },
    kind: { display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '2px 6px', fontSize: '12.5px', lineHeight: '18px', fontWeight: '600', color: 'var(--ink-muted)', '& .m-icon': { fontSize: '16px' } },
    in: { fontWeight: '400', marginInlineStart: '8px' },
    title: { fontSize: '15px', lineHeight: '20px', fontWeight: '450', overflowWrap: 'anywhere' },
    excerpt: { fontSize: '13.5px', lineHeight: '19px', color: 'var(--ink-muted)', display: '-webkit-box', WebkitLineClamp: '2', WebkitBoxOrient: 'vertical', overflow: 'hidden' },
    side: { display: 'grid', justifyItems: 'end', gap: '6px', fontSize: '13px', color: 'var(--ink-muted)', fontVariantNumeric: 'tabular-nums', whiteSpace: 'nowrap' },
  } as any,
  variants: {
    kind: {
      turn: { kind: { color: 'var(--accent-ink)' } },
      mention: {}, application: {}, post: {}, decision: {}, draft: {},
    },
    unread: { true: { root: { '&::before': { backgroundColor: 'var(--accent)' } }, title: { fontWeight: '650' } }, false: {} },
  } as any,
  staticCss: [{ kind: ['*'], unread: ['*'] }],
})

/* ---------------------------------------------------------------
   Character card: World-framed; the owning account stays visible
   --------------------------------------------------------------- */
export const characterCard = defineSlotRecipe({
  className: 'm-character',
  slots: ['root', 'portrait', 'body', 'name', 'epithet', 'facts', 'owner', 'foot'],
  base: {
    root: {
      ...framed('var(--surface-raised)'), position: 'relative', display: 'grid', gridTemplateColumns: '112px minmax(0, 1fr)', gap: '18px',
      padding: 'max(16px, var(--orn-pad, 0px))', boxShadow: 'var(--elevation-2), var(--edge-light)', fontFamily: 'ui',
      '[data-frame=double] &': { outline: '1px solid var(--line)', outlineOffset: '-6px' },
      '@container (max-width: 420px)': { gridTemplateColumns: '80px minmax(0, 1fr)', gap: '14px' },
    },
    portrait: {
      aspectRatio: '3 / 4', width: '100%', overflow: 'hidden', borderRadius: 'md', cornerShape: 'var(--corner-shape)',
      backgroundColor: 'var(--surface-quiet)', boxShadow: 'inset 0 0 0 1px var(--line)', '& img': { width: '100%', height: '100%', objectFit: 'cover', display: 'block' },
    },
    body: { minWidth: '0', display: 'grid', alignContent: 'start', gap: '4px' },
    name: {
      margin: '0', fontFamily: 'name', fontSize: 'calc(22px * var(--name-adjust, 1))', lineHeight: '1.15', fontWeight: '600',
      textTransform: 'var(--name-case, none)', letterSpacing: 'var(--name-tracking, 0)', fontVariationSettings: 'var(--name-axes, normal)',
    },
    epithet: { fontSize: '13.5px', lineHeight: '18px', color: 'var(--ink-muted)' },
    facts: {
      display: 'grid', gridTemplateColumns: 'auto minmax(0, 1fr)', gap: '4px 14px', margin: '10px 0 0', fontSize: '13px', lineHeight: '18px',
      '& dt': { color: 'var(--ink-muted)' }, '& dd': { margin: '0', color: 'var(--ink)', fontVariantNumeric: 'var(--numerals, normal)' },
    },
    owner: { display: 'flex', alignItems: 'center', gap: '6px', marginTop: '10px', fontSize: '13px', color: 'var(--ink-muted)' },
    foot: { gridColumn: '1 / -1', display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '8px', paddingTop: '12px', boxShadow: 'inset 0 1px 0 var(--line)' },
  } as any,
})

/* ---------------------------------------------------------------
   Lane filter: in character, out of character, or both. Two toggles
   in one split button; one is always on (see Motet.nextLane).
   --------------------------------------------------------------- */
export const lanes = defineSlotRecipe({
  className: 'm-lanes',
  description: 'Which lanes a Scene shows: in character, out of character or both.',
  slots: ['root', 'item'],
  base: {
    root: {
      display: 'inline-flex', alignItems: 'center', gap: '2px', padding: '2px', flex: 'none',
      border: '1px solid var(--line-strong)', borderRadius: 'control', cornerShape: 'var(--corner-shape-control)', backgroundColor: 'var(--surface-raised)',
    },
    item: {
      position: 'relative', display: 'inline-grid', placeItems: 'center', width: '36px', height: 'calc(var(--control-h) - 10px)', padding: '0', border: '0',
      borderRadius: 'calc(var(--radius-control) - 3px)', cornerShape: 'var(--corner-shape-control)', backgroundColor: 'transparent', color: 'var(--ink-muted)', cursor: 'pointer',
      backgroundImage: 'linear-gradient(var(--state, transparent), var(--state, transparent))',
      transition: motion.effects('background-color, color'),
      '& .m-icon': { fontSize: '19px' },
      '@media (hover: hover)': { '&:hover': { '--state': 'color-mix(in oklab, currentColor 8%, transparent)' } },
      '&[aria-pressed=true]': { backgroundColor: 'var(--accent-soft)', color: 'var(--on-accent-soft)', '& .m-icon': { fontVariationSettings: '"FILL" 1, "wght" var(--icon-wght), "GRAD" var(--icon-grad), "opsz" 20' } },
      '&:focus-visible': { outline: '2px solid var(--focus)', outlineOffset: '2px', zIndex: '1' },
      '@media (pointer: coarse)': { '&::before': { content: '""', position: 'absolute', inset: '-6px -2px' } },
      '@media (forced-colors: active)': { '&[aria-pressed=true]': { backgroundColor: 'Highlight', color: 'HighlightText' } },
    },
  } as any,
})

/* Motet's own pages (Home, Inbox, Search, Settings, profiles): one solid surface under the chrome
   islands, with no frame and no art; the World cards and rows on it carry the colour. Side
   navigation and filters sit in a quiet panel. A single-task page (sign in) puts one card on the
   canvas instead. */
export const page = defineSlotRecipe({
  className: 'm-page',
  description: "Motet's own pages: a solid surface, no frame; side menus and filters in a quiet panel.",
  slots: ['root', 'main', 'panel', 'navItem', 'card'],
  base: {
    root: { minHeight: '100vh', backgroundColor: 'var(--surface)', color: 'var(--ink)', fontFamily: 'ui', fontSize: '14px', lineHeight: '20px' },
    main: {
      boxSizing: 'border-box', width: '100%', maxWidth: 'var(--page-max, 1200px)', marginInline: 'auto', containerType: 'inline-size',
      paddingTop: 'calc(var(--chrome-offset, 72px) + 16px)', paddingBottom: '64px',
      paddingInline: 'max(16px, env(safe-area-inset-left, 0px)) max(16px, env(safe-area-inset-right, 0px))',
      '@media (min-width: 760px)': { paddingInline: '24px' },
    },
    panel: {
      boxSizing: 'border-box', display: 'flex', flexDirection: 'column', gap: '4px', minWidth: '0', padding: '12px',
      backgroundColor: 'var(--surface-quiet)', border: '1px solid var(--line)', borderRadius: 'lg', cornerShape: 'var(--corner-shape)',
    },
    navItem: {
      position: 'relative', display: 'flex', alignItems: 'center', gap: '12px', width: '100%', minHeight: '40px', paddingInline: '10px', paddingBlock: '6px',
      border: '0', borderRadius: 'md', cornerShape: 'var(--corner-shape-control)', backgroundColor: 'transparent', color: 'var(--ink)',
      fontFamily: 'ui', fontSize: '14px', lineHeight: '20px', fontWeight: '500', textAlign: 'start', textDecoration: 'none', cursor: 'pointer',
      transition: motion.effects('background-color, color'),
      '& > .m-icon': { color: 'var(--ink-muted)' },
      '& > .m-count': { marginInlineStart: 'auto' },
      '@media (hover: hover)': { '&:hover': { backgroundColor: 'color-mix(in oklab, currentColor 7%, transparent)' } },
      '&[aria-current=page], &[aria-current=true]': {
        backgroundColor: 'var(--accent-soft)', color: 'var(--on-accent-soft)', fontWeight: '600',
        '& > .m-icon': { color: 'inherit', fontVariationSettings: '"FILL" 1, "wght" var(--icon-wght), "GRAD" var(--icon-grad), "opsz" 24' },
      },
      '@media (pointer: coarse)': { minHeight: '44px' },
    },
    card: {
      boxSizing: 'border-box', width: 'min(440px, 100%)', display: 'flex', flexDirection: 'column', gap: '20px', padding: 'clamp(20px, 5vw, 36px)',
      backgroundColor: 'var(--surface-raised)', border: '1px solid var(--line)', borderRadius: 'xl', cornerShape: 'var(--corner-shape)',
      boxShadow: 'var(--elevation-2), var(--edge-light)',
    },
  } as any,
  variants: {
    ground: { surface: {}, canvas: { root: { backgroundColor: 'var(--canvas)' } } },
  } as any,
  staticCss: [{ ground: ['*'] }],
})

/* ---------------------------------------------------------------
   Scene events: what happens between posts. A roll, a roll request,
   a round starting, a tool's output. They are not prose, so they
   never sit inside a post: each is its own block in the stream, in
   time order, on a quiet card aligned with the prose column. The
   account that caused it and the tool that made it are always shown;
   the body is a slot for dice today and for any first-party tool or
   sandboxed tile later (`frame`). `data-lane="ooc"` puts an event in
   the out-of-character lane; the default is in character.
   --------------------------------------------------------------- */
export const event = defineSlotRecipe({
  className: 'm-event',
  description: 'Something that happened between posts: a roll, a request, a tool result.',
  slots: ['root', 'marker', 'card', 'header', 'title', 'detail', 'time', 'body', 'footer', 'source', 'actions', 'frame'],
  base: {
    root: {
      position: 'relative', display: 'grid', gridTemplateColumns: 'var(--portrait-col, 56px) minmax(0, 1fr)', columnGap: '20px',
      paddingBlock: 'calc(var(--post-gap) / 4)',
      '[data-portrait=inline] &, [data-portrait=none] &, [data-post=dossier] &': { gridTemplateColumns: 'minmax(0, 1fr)' },
      '@container stream (max-width: 600px)': { gridTemplateColumns: 'minmax(0, 1fr)' },
    },
    marker: {
      justifySelf: 'center', alignSelf: 'start', display: 'grid', placeItems: 'center', width: '32px', height: '32px', marginTop: '10px',
      borderRadius: '50%', cornerShape: 'round', backgroundColor: 'var(--surface-quiet)', boxShadow: 'inset 0 0 0 1px var(--line)', color: 'var(--ink-muted)',
      '& .m-icon': { fontSize: '18px' },
      '[data-portrait=inline] &, [data-portrait=none] &, [data-post=dossier] &': { display: 'none' },
      '@container stream (max-width: 600px)': { display: 'none' },
    },
    card: {
      minWidth: '0', maxWidth: 'calc(68ch + 34px)', display: 'grid', gap: '10px', paddingInline: '16px', paddingBlock: '12px',
      backgroundColor: 'var(--surface-quiet)', border: '1px solid var(--line)', borderRadius: 'md', cornerShape: 'var(--corner-shape)',
      fontFamily: 'ui', fontSize: '14px', lineHeight: '20px', color: 'var(--ink)',
    },
    header: { display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) auto', columnGap: '12px', rowGap: '2px', alignItems: 'baseline' },
    title: {
      display: 'flex', flexWrap: 'wrap', alignItems: 'baseline', gap: '2px 10px', margin: '0', fontSize: '14px', lineHeight: '20px', fontWeight: '600',
      '& code': { fontFamily: 'mono', fontSize: '13px', fontWeight: '400', color: 'var(--ink-muted)' },
    },
    detail: {
      gridColumn: '1 / -1', display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '2px 8px', fontSize: '13px', lineHeight: '18px', color: 'var(--ink-muted)',
      '& .m-icon': { fontSize: '16px' },
    },
    time: { fontSize: '13px', color: 'var(--ink-muted)', fontVariantNumeric: 'tabular-nums', whiteSpace: 'nowrap' },
    body: { minWidth: '0' },
    footer: { display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '8px 12px', paddingTop: '8px', boxShadow: 'inset 0 1px 0 var(--line)' },
    source: {
      display: 'inline-flex', alignItems: 'center', gap: '6px', minWidth: '0', fontSize: '12.5px', lineHeight: '16px', color: 'var(--ink-muted)',
      '& .m-icon': { fontSize: '16px' }, '& a': { color: 'inherit' },
    },
    actions: { display: 'flex', flexWrap: 'wrap', gap: '8px', marginInlineStart: 'auto' },
    frame: {
      display: 'block', width: '100%', height: 'var(--tile-h, 240px)', minHeight: '120px', maxHeight: '480px', border: '0',
      borderRadius: 'sm', backgroundColor: 'var(--surface)', boxShadow: 'inset 0 0 0 1px var(--line)',
    },
  } as any,
  variants: {
    kind: {
      roll: {},
      tool: {},
      /* asks someone to act: raised like the composer, marked like "Your turn" */
      request: {
        card: { backgroundColor: 'var(--surface-raised)', borderColor: 'var(--accent-ink)', boxShadow: 'var(--elevation-1), var(--edge-light)' },
        marker: { backgroundColor: 'var(--accent-soft)', color: 'var(--on-accent-soft)', boxShadow: 'none' },
      },
      /* a one-line change of state: a round starts, the cast changes, the scene pauses */
      system: {
        root: { paddingBlock: '4px' },
        card: { backgroundColor: 'transparent', border: '0', paddingInline: '0', paddingBlock: '4px', gap: '0' },
        marker: { width: '24px', height: '24px', marginTop: '2px', '& .m-icon': { fontSize: '16px' } },
        title: { fontWeight: '500' },
      },
    },
  } as any,
  staticCss: [{ kind: ['*'] }],
})
