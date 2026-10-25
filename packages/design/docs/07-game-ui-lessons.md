# Game UI lessons

Games are the best examples of interfaces styled to belong to a fictional world. Motet borrows their methods, not their screens. In the usual terms (diegetic, non-diegetic, spatial, meta, after Fagerholt and Lorentzon), Motet's UI is never diegetic: no character can see a Reply button. A World instead styles a non-diegetic interface as if it were made inside the fiction, the way a Pip-Boy, a 1979 terminal or an immigration desk would make it.

## What each game taught

| Game | What it does | What Motet takes |
| --- | --- | --- |
| Alien: Isolation | Holds itself to "retro lo-fi": no technology that "couldn't have existed on-set in 1979". Interfaces were filmed on tube TVs and VHS. Film grain is an adjustable option | **One medium per World.** Kepler Relay uses one phosphor hue, mono labels and log rows. Scan lines live in its art, not its controls. Grain is a material level, and readers can turn decoration off |
| Fallout (Pip-Boy) | A single-hue terminal the player can recolour | Monochrome Worlds are valid. The reader keeps control of mode and contrast |
| Elite Dangerous, Star Citizen | Translucent, hairline-edged panels float over the scene. In Elite, players remap the whole cockpit HUD through one 3×3 colour matrix, with community editors to help | **Veil and frosted sheets**: straight-edged translucent panes over the art, at an opacity proved legible against it. Reader authority over colour, except that Motet keeps reserved hues (danger, caution, focus, selection) out of any remap |
| Cyberpunk 2077 | Clipped, asymmetrical panels and small accent bars carry the identity. The critique: one red for everything flattens hierarchy, decoration looks identical to data, a non-clickable tag with a cut corner reads as a button, and every shard shares one reading UI | **Profiles and segment marks** for hard-edged Worlds, on large surfaces only. Decoration has its own `ornament` role, separate from the accent. Controls never take a profile, and flags never look like buttons. Posts, OOC, rolls and lore each read differently |
| Papers, Please, Orwell | The interface is a desk of documents: labelled fields quieter than the data they hold, in a deliberately limited space | **Dossier** posts. A form header strip and label/value fields for strategy and investigation Worlds |
| Pentiment | Typefaces stand in for voices, by class and education. An accessibility setting applies one legible face to every voice | The display face belongs to the World. The reading face belongs to the reader, and one setting makes everything plain |
| Disco Elysium | Dialogue tumbles up as a single skimmable column, modelled on a Twitter feed | One stream column. The `log` layout for Worlds whose fiction is a transcript |
| Persona 5 | Strong hierarchy through alignment and proximity, and a selection colour that stands apart from the palette. Each menu is its own program and design, which its director calls "really annoying to do" | Focus and selection are reserved and never take the World's accent. A World chooses axes, not bespoke screens |
| Dead Space | Health sits on the character's spine, fully diegetic. The holographic map failed at navigation, and a projected locator was added | **Function first.** World styling never moves or replaces a control. Navigation and safety are Motet's |
| Hacknet, NITE Team 4, Orwell | The whole game is a computer, so the whole UI can be diegetic | This doesn't transfer to fantasy or horror. Those Worlds use object metaphors (chapters, plates, documents) rather than screens |

## Rules that follow

1. **Function first.** A World changes how a control looks, never where it is, what it says or how it behaves (Andrews: "functionality preservation").
2. **Decoration gets its own colour and sits lower.** `ornament` is never the accent. Ornaments are smaller and quieter than anything you can act on.
3. **Shape must not lie.** Chamfers and marks belong on large surfaces. Small things you can't press (flags, tags, counts) never borrow a button's shape.
4. **Pick one medium and keep it.** Phosphor, paper, steel or vellum: one constraint, applied to type, edge, ornament and composition together, beats a collage.
5. **Different content reads differently.** A post, an OOC note, a roll and a lore page never share one undifferentiated treatment.
6. **Readers can always opt out.** Reading face, size, contrast, motion, transparency and decoration are theirs, and "Use Motet appearance" is one step away.
7. **A few strong axes, not bespoke screens.** Persona's menus show the ceiling and the cost. Motet gives authors axes that hold up on every screen, including the ones they never designed.

## Sources

- [Cyberpunk 2077 UX/UI critique](https://interfaceingame.com/articles/cyberpunk-2077-ux-ui-critique), Interface In Game
- [User Interface in Cyberpunk 2077: Challenges and Optimizations](https://gdcvault.com/play/1034160/User-Interface-in-Cyberpunk-2077), Arkadiusz Antonik, GDC Showcase 2023
- [Tube TVs, VCRs and magnets give Alien: Isolation its signature look](https://www.engadget.com/2014-10-07-alien-isolation-launch-interview.html), Engadget
- [Alien: Isolation, lo-fi sci-fi](https://hudsandguis.com/home/2014/06/04/alien-isolation), HUDS+GUIS
- [Custom fonts for Pentiment](https://lettermatic.com/custom/pentiment), Lettermatic
- [Disco Elysium: working on UI design](https://80.lv/articles/disco-elysium-working-on-ui-design), 80 Level
- [Visual design of games: Persona 5](https://mechanicsofmagic.com/2022/04/23/visual-design-of-games-persona-5/), Mechanics of Magic
- [Hashino on Persona and Metaphor menus](https://kotaku.com/-metaphor-refantazio-persona-5-menu-ui-hashino-1851667088), Kotaku
- [Design details in Papers, Please](https://mechanicsofmagic.com/2021/06/05/design-details-in-papers-please/), Mechanics of Magic
- [Dead Space 2: diegetic interface design](https://www.hudsandguis.com/2012/08/22/dead-space-2-diegetic-interface-design), HUDS+GUIS
- [Game UI discoveries: what players want](https://www.gamedeveloper.com/design/game-ui-discoveries-what-players-want), Marcus Andrews, citing Fagerholt and Lorentzon, *Beyond the HUD* (Chalmers, 2009)
- [Set up your Elite Dangerous HUD colours](https://www.keengamer.com/articles/guides/elite-dangerous-set-up-your-hud-colours-ui-as-you-want/), KeenGamer, and the [HUD colour editor](https://arkku.com/elite/hud_editor)
- [Cyberpunk 2077 user interface, part 2](https://www.behance.net/gallery/133185623/Cyberpunk-2077User-Interface-(Part-2)), Behance
- [DiceFont](https://github.com/fponticelli/dicefont), Franco Ponticelli
