import { definePreset } from '@pandacss/dev'
import { globalCss, keyframes } from './global'
import { semanticTokens, tokens } from './tokens'
import { avatar, button, chip, dialog, field, flag, iconButton, menu, segmented, switchRecipe, tabs, tooltip } from './recipes/controls'
import { characterCard, chrome, composer, divider, event, inboxItem, lanes, masthead, page, post, roll, sceneRow, world } from './recipes/patterns'
import { calendar, checkbox, fieldset, inlineSelect, input, listbox, radio, select, slider, tags, textarea, upload, colorpicker } from './recipes/forms'
import { accordion, breadcrumb, collapsible, pagination, steps, tree } from './recipes/disclosure'
import { drawer, hovercard, popover, toast } from './recipes/overlays'
import { alert, datalist, progress, ring, skeleton } from './recipes/feedback'

/*
 * Motet preset for Panda CSS v2.
 *
 *   import base from '@pandacss/preset-base'
 *   import motet from '@motet/design/panda'
 *   export default defineConfig({ presets: [base, motet], cssVarRoot: motetCssVarRoot, … })
 *
 * The preset owns static styling. World appearance is runtime data: Motet.applyWorld()
 * writes custom properties on a [data-world] element and every recipe follows.
 */
export const motetCssVarRoot = ':where(:root, :host, [data-world], .m-chrome__island)'

export default definePreset({
  name: '@motet/design',
  conditions: {
    extend: {
      reduceTransparency: '@media (prefers-reduced-transparency: reduce)',
      worldDark: '[data-mode=dark] &',
      coarse: '@media (pointer: coarse)',
      forcedColors: '@media (forced-colors: active)',
    },
  },
  globalCss: globalCss as any,
  /* typed so the scroll-row edge fades interpolate */
  globalVars: {
    '--m-fade-start': { syntax: '<length>', inherits: false, initialValue: '0px' },
    '--m-fade-end': { syntax: '<length>', inherits: false, initialValue: '0px' },
  } as any,
  theme: {
    extend: {
      tokens: tokens as any,
      semanticTokens: semanticTokens as any,
      keyframes,
      recipes: { button, iconButton, chip, avatar, tooltip, flag, skeleton },
      slotRecipes: {
        segmented, switch: switchRecipe, field, tabs, menu, dialog, world, post, ornamentDivider: divider, masthead, chrome, composer, roll, sceneRow, inboxItem, characterCard, lanes, page, event,
        checkbox, radio, fieldset, input, textarea, listbox, select, inlineSelect, tags, calendar, slider, upload, colorpicker,
        accordion, collapsible, tree, pagination, breadcrumb, steps,
        popover, hovercard, drawer, toast,
        alert, progress, ring, datalist,
      },
    },
  },
})
