# Apothecary Ink — The Design System

> **Status: locked.** Apothecary Ink is the visual direction for the Boiling Point
> graphical client (`web-client/`, PixiJS — see `adopt-pixi-client`). This document
> is the canonical and **sole** record of the system — the HTML exploration tiles it
> was distilled from have been retired, and all assets will be drafted fresh against
> the descriptions below.
>
> ⚠ **Naming:** "Apothecary Ink" (this visual direction) and the **Apothecary**
> (the boom2 deck-drafting mechanic, `openspec/changes/boom2-apothecary/`) are
> independent things that happen to share a word. The collision is fortunate —
> see [Forward hooks](#forward-hooks-boom2) — but never abbreviate this system
> to "Apothecary".

---

## 1. Concept

**An illuminated recipe-book of ruin.** Iron-gall ink, burnished gold leaf, and
wax-seal pigments on aged parchment — a medieval manuscript that happens to contain
a frantic, dangerous game. The premium, heirloom reading of Boiling Point: a
beautiful boxed game you keep on a shelf and are slightly afraid of.

It is the only **warm light** theme on the bench, and that is the point: parchment
makes the four player jewels read as *pigment*, gold read as *value*, and the boom
read as a *stain* spreading through the page.

> **Note — supersedes part of `02_game-design.md` §15.** The art-direction reference
> there ("Arcane Punk Alchemy", *dark brass/iron base*) predates this decision. The
> spirit carries over — frantic, slightly dangerous workshop; jewels pop; readability
> law — but the substrate is now **light parchment**, not dark brass. §15's two hard
> rules are restated in [§9 Invariants](#9-invariants-hard-rules) and remain binding.

## 2. The organizing principle — one book, three hands

Medieval books were made by a workshop, not a person, and **the division of labour
is the design system**. Every visual asset exists in up to three *registers*, each
owned by a craftsman, and **context — not taste — decides which hand is used**:

| Hand | Register | Material | Owns | Cost |
|---|---|---|---|---|
| ✒ **The Scribe** | the quiet one | iron-gall line, vermilion rubrication | UI chrome, small sizes, dense panels, monochrome contexts, loading states. Line art *draws itself* on load. | cheapest — one stroked path set, tints to any ground |
| ❦ **The Illuminator** | **the default** | gold leaf, lapis, vermilion | the **brand register**: cards, logos, seals, devices — anything a player should covet | mid — gradients + simple fills |
| ✴ **The Conjurer** | the loud one | the page, alive | **moments only**: main menu, the depile, the boom, victory. Wheels turn, vines grow, imps blink. | dearest — reserved, never ambient |

Implementation rule of thumb: *if you're unsure which hand an element belongs to,
it's the Scribe's; if it's something the player owns or wants, it's the
Illuminator's; the Conjurer must justify himself against §9.1 every time.*

## 3. Foundations — tokens

One token source must feed both render targets (Pixi canvas constants **and** DOM
overlay CSS custom properties) — generate both from a single module so they cannot
drift, mirroring the protocol-codegen philosophy.

### 3.1 The substrate (paper & ink)

| Token | Hex | Role |
|---|---|---|
| `paper-0` | `#EFE6D0` | the page — app background |
| `paper-1` | `#E7DCC1` | panel ground |
| `paper-2` | `#DDD0AF` | sunk / recessed ground |
| `edge` | `#C9B78E` | panel & card edges |
| `ink` | `#2A2118` | iron-gall — headings, primary text |
| `ink-soft` | `#5A4A35` | body text |
| `ink-dim` | `#8A7657` | captions, de-emphasis |

### 3.2 Metals & pigments

| Token | Hex | Role |
|---|---|---|
| `gold` | `#A9801F` | gold leaf — the value metal |
| `gold-hi` | `#D8B24A` | gilt highlight |
| `gold-pale` | `#F0D98A` | gilt top-light (gradient stop, charges on shields) |
| `gold-deep` | `#7A5C12` | gilt shadow (gradient stop, gilt strokes) |
| `wax` | `#9E2B1E` | wax-seal vermilion — **volatility & urgency** |
| `wax-hot` | `#C43A22` | heated vermilion — active/hot states |
| `boom` | `#8F2012` | the explosion — **boom contexts only** |
| `lapis` | `#2F5AA8` | lapis ultramarine — illuminated grounds (volatility plaque) |
| `lapis-deep` | `#1C3C78` | lapis shadow |
| `vert` | `#2C7D4F` | verdigris green — the brew's surface |

Gilt anything = the 3-stop gradient `gold-pale → gold → gold-deep` (vertical),
stroked in `gold-deep`.

### 3.3 The four jewels (brand constant, re-tinted)

The jewels are **identity, not theming** — canonical hues live in
`tui-client/src/palette.rs`; every theme re-tints them to sit on its ground but
they stay recognizably the four. Apothecary Ink presses them as earthier
*illuminated pigments* on parchment:

| Jewel | Glyph | Canonical (TUI) | Apothecary pigment |
|---|---|---|---|
| Ruby | ▲ | `#DC3246` | `#B23149` |
| Sapphire | ♥ | `#3C78E6` | `#2F5AA8` *(= lapis)* |
| Emerald | ● | `#32BE6E` | `#2C7D4F` *(= vert)* |
| Amethyst | ■ | `#B464DC` | `#7A4B9C` |
| Wild | ✦ (TUI: ★) | `#BEBEBE` | `#6B5D45` (umber, colorless) |

Sapphire and Emerald deliberately share hues with the lapis/verdigris pigments —
the jewels *are* the pigments of the book.

### 3.4 Depth, edges, texture

- **Shadow** (one shadow, everywhere): `0 16px 40px -18px rgba(40,25,10,.55)`
- **Hairline**: `1px solid rgba(42,33,24,.22)`
- **Foxing** — full-page fractal-noise grain (`feTurbulence`, baseFrequency 0.6,
  3 octaves), opacity `.06`, multiply blend. The page is never flat white.
- **Candle warmth** — a radial pool of warm light top-right
  (`rgba(255,210,120,.22)`, screen blend) that *breathes* on the flicker keyframe.
- **Corner radii are small**: 4–8px. This is a book, not an app; nothing is a pill
  except chips/badges.
- Page gradients: the page is lit, not flat — a soft radial pool of warm light
  top-left (`rgba(255,250,235,.6)` fading by 50%) and a tan bloom upper-right
  (`rgba(180,150,90,.26)` fading by 55%) over `paper-0`.

## 4. Typography

| Role | Face | Usage |
|---|---|---|
| **Display** | IM Fell English SC | titles, section heads, numerals (stamped figures), letter-spaced uppercase labels/eyebrows |
| **Body** | EB Garamond (400/500/600 + italic) | rules text, effect text, flavor (italic), captions |
| **Flourish** | UnifrakturMaguntia | drop caps, the boom word, sigils, Scribe-register volatility numerals — *never running text* |
| **Script** | Pinyon Script | colophons, handwritten asides — decoration only |

Conventions:

- Eyebrow/section labels: IM Fell English SC, uppercase, letter-spacing `.18–.34em`,
  small (12–15px), in `wax` with a `❧` fleuron in `gold`.
- Flavor and rules-asides are EB Garamond *italic* in `ink-soft`.
- Drop caps (UnifrakturMaguntia, in `wax`) open ledes and flavor blocks.
- Numerals that matter (volatility, timer, scores) are display faces, big, in `wax`.
- Body line-height ≈ 1.6; body color is `ink-soft`, never pure `ink` (headings own `ink`).
- **Fonts must be self-hosted woff2** in the client (no Google Fonts at view time;
  the client must be offline-clean) and first paint should gate on
  `document.fonts.ready`.
- Localization note (EFIGS planned): all four faces must be verified for the EFIGS
  diacritic set; UnifrakturMaguntia is decorative-only partly for this reason.

## 5. Semantic color — what means what

These mappings are law; semantic colors do not cross roles:

- **`wax` = volatility, heat, urgency.** Volatility numerals, the timer ring, the
  fuse, hot labels.
- **`gold` = value, premium, leadership.** Points bezants, frames, the leader's `✠`,
  anything ownable.
- **`boom` = the explosion.** Only ever seen when a pot tips or in its aftermath.
- **Jewel colors = card/player identity.** Never used decoratively.
- **`lapis`/`vert`** are grounds (illuminated plaques, the brew surface), not signals.

## 6. The components

### 6.1 The card — *enluminure* (Illuminator face, the shipping default)

The readability law **Volatility > Color > Points > Effect** governs every face.
Aspect ratio **5:7**, parchment ground (`#F5ECD6 → #E7DCBD` vertical), gilt double
frame with **gold bezants** at the four edge midpoints.

| Priority | Element | Treatment |
|---|---|---|
| 1 — loudest | **Volatility** | an illuminated drop-capital: blackletter numeral in `gold-pale` on a **lapis plaque** with gilt border, top-left |
| 2 | **Color** | a moulded **wax seal** in the jewel pigment, top-right, bearing the jewel glyph in off-white |
| 3 | **Points** | **gold bezants** (filled = earned, recessed = empty) in a bottom row above a gilt hairline |
| 4 — quietest | **Effect** | EB Garamond, small, centered — the recipe note |

Art sits in a **gilt roundel** (circular miniature) centre-card; name in IM Fell
English SC below it; type line in EB Garamond italic in `wax`. A slow gold-leaf
sheen may sweep the frame of a *selected/playable* card.

Companion faces:

- **Scribe face ("the Chancery Hand")** — double ink rule, rubricated blackletter
  volatility numeral, flat tempera seal, woodcut roundel, inked points. For cheap
  contexts: wide hand fans, history lists, tiny sizes.
- **Conjurer face ("Rota Fortunæ")** — the medieval Wheel of Fortune *as* the card
  frame: the wheel slowly turns, a horned imp rides the rim, volatility waits at
  the still hub. Deluxe/promo/signature contexts only. **Deferred** (see §11).

### 6.2 Asset register (the starred picks)

Each asset has a setting per hand; the ★ pick is the brand workhorse.

| Asset | ✒ Scribe | ❦ Illuminator ★ | ✴ Conjurer |
|---|---|---|---|
| **Cauldron mark** | Woodcut Line — self-drawing outline; doubles as the loading spinner | **Gilt Silhouette** — burnished gold belly, lapis-bead boil; the logo from app icon to favicon | Alchemical Orrery — counter-rotating elemental rings; the menu mark |
| **Initial "B"** | Rubricated B — blackletter + red cadel penwork; chapter headings, settings, rulebook | **Inhabited Initial** — gold letter on lapis diaper; a cauldron simmers in the top counter, an imp blinks below; the storybook logo | Zoomorphic Knotwork — Kells-style interlace ending in beasts; draws itself on load |
| **Mascot / drolleries** | Snail-Herald — line snail bearing a banner (tints to any faction) | **The Pot-Imp** — copper-skinned imp hugging a gilt pot; *the* mascot, sticker-grade | Knight-vs-Snail Joust — the 600-year-old gag, animated; lobby waits & between rounds |
| **Border / frame** | Rinceaux Penwork — thin ivy scroll + red berries; edges of dense UI panels | **Acanthus & Bezant** — bold leaves on a double gilt rule, bezant terminals; the system default frame, tiles to any length | Self-Growing Vine — stem draws on, leaves unfurl, the snail arrives last; big reveals |
| **Faction device** | Wax Seal — pressed disc, dashed-ring emboss; chat, letters, system notices | **Heraldic Shield** — jewel field, gilt edge, `gold-pale` charge; seat markers, score ledger, player lists | Faceted Gem — cut stone with sparkle; victory screens, the pot-scoop |

All assets are **hand-drafted inline SVG**, authored fresh against the descriptions
above as individual `.svg` files in the client. No raster art, no binary assets
(constitution §II: everything agent-writable).

### 6.3 Table chrome

- **Panels**: `paper-1 → paper-2` vertical gradient, `edge` border, the one shadow,
  panel titles as eyebrows in `wax`.
- **Wave timer**: a conic ring in `wax` over an ink-faint track, display numeral in
  the centre, wave label as an eyebrow.
- **Scoreboard**: ledger rows on faint warm white, each with a 4px left border in
  the player's pigment, a moulded seat-dot, name in EB Garamond, score in IM Fell
  English SC. The leader's name carries a gold `✠`.
- **Modifier tokens**: parchment roundels with a 2px `gold` rim and a sepia-tinted
  glyph; italic caption beneath.
- **Emotes**: parchment roundels, hairline edge; hover = small lift + gold ring.
- **Depile chips**: miniature recipe-card chips (jewel seat-dot + volatility numeral
  in `wax`); the tipping card gets a `boom` ring, crack shadow, and an italic
  "✦ tipped" annotation.

## 7. The screens (starred layouts)

The chosen arrangement per screen (alternates were considered and dropped):

- **The Round — "Codex Spread"**: the screen as an open book; ledger (scores,
  modifiers) as the left page, the brew and your hand as the right.
- **The Depile — "Page-Turn"**: each revealed card is a turning leaf, centre-stage,
  with the **fuse** climbing beneath; prior/next cards stacked at the wings.
- **The Boom — "Boom → Ledger"**: the blackletter boom word floods the page top
  (vermilion stain radial behind it), then the score ledger unfurls below.

## 8. Motion — a small, period-honest vocabulary

**Restraint is the brief: manuscripts don't bounce.** The system has exactly
**five verbs**; every Conjurer set-piece is a composition of them.

| Verb | What | Where | Reference timing |
|---|---|---|---|
| **Gold-leaf sheen** | a slow highlight sweep across gilt | gilt text, frames, selected cards | 5–6s linear loop |
| **Candle flicker** | the ambient warmth subtly pulses | page warmth, flame glyphs | ~5.5s organic loop |
| **The simmer** | bubbles rise & pop in the brew | cauldron — **ambient only, constant rate** (§9.1) | 2.4–2.8s staggered |
| **Ink draws itself** | line art renders stroke-first | Scribe assets on load/entry | 1.4s ease, `stroke-dashoffset` |
| **The fuse** | volatility climbs toward the boil | the depile only | tied to depile pacing |

Micro-behaviors used inside Conjurer compositions (not standalone verbs): `unfurl`
(1.1s overshoot bezier `.2,1.4,.5,1`), `sway`, `breathe`, `twinkle`, `sparkle`,
slow `spin` (30–60s — the orrery and the Rota). Easing default: `ease-in-out`;
nothing snappier than the unfurl overshoot.

**`prefers-reduced-motion: reduce`** kills *all* animation and resolves drawn
strokes and unfurls to their final state. Non-negotiable.

The depile is the one place motion carries game information — clients buffer the
end-of-round resolution and animate it on a debit against the next wave's timer
(see the resolution-pacing design in the TUI client); the fuse verb *is* that
animation.

## 9. Invariants (hard rules)

1. **No mid-round cauldron cues** (`02_game-design.md` §4): the cauldron has no
   rumble, glow, or any state-dependent tell during play. The simmer runs at a
   **constant rate, never parameterized by pot volatility**. All drama is
   concentrated in the **depile** and the **boom** — which is exactly why the
   Conjurer is reserved for those moments.
2. **Card readability law**: Volatility > Color > Points > Effect, on every face,
   in every hand, at every size.
3. **The jewels are brand constants** — names, glyphs (▲ ♥ ● ■, plus Wild),
   four-ness. Re-tinting for the parchment ground is allowed; identity changes
   are not.
4. **Semantic colors don't cross** (§5): gold never signals danger, wax never
   decorates, boom-red appears only at the boom.
5. **Reduced motion is honored everywhere** (§8).
6. **Pure renderer**: nothing in this system infers or extrapolates game state —
   it renders what the server says (constitution §I).

## 10. Implementation notes (PixiJS + DOM overlay)

- **Tokens**: author once (e.g. `web-client/src/theme/tokens.ts` or a JSON source),
  generate the CSS custom-property sheet for the DOM overlay from it. Canvas and
  overlay must read the same values.
- **Assets**: author the inline-SVG masters in `web-client/assets/` as `.svg`
  files. For Pixi, render SVG → texture at load (at 2× for high-DPI); keep
  Scribe-register "draws itself" pieces as DOM/SVG where feasible, since
  `stroke-dashoffset` is free there.
- **Text**: per `adopt-pixi-client`, selectable/accessible text (room code, chat,
  names, scores) lives in the thin DOM overlay — that's also where EB Garamond
  body text will render best; canvas text is for display numerals and staged
  moments.
- **Fonts**: self-host woff2 (4 families); gate first paint on `document.fonts.ready`.
- **Texture layers**: foxing + candle warmth are two cheap fullscreen layers
  (one multiply, one screen); in Pixi, a noise texture sprite + a radial gradient
  sprite with blend modes.
- **Visual testing**: a Playwright screenshot harness is the §II visual layer —
  wait on `document.fonts.ready` before each capture (so screenshots validate the
  real type, not fallbacks), fail loud on any console/page error, and shoot
  desktop + mobile viewports.

## 11. Scope — what ships first vs. the flourishes

Per constitution §III, the minimum implementable set is everything the *game*
needs; the Conjurer's showpieces are deliberate later-polish.

**v1 set** — tokens & texture; the four type roles; the Illuminator card face +
Scribe card face; Gilt Silhouette cauldron (logo/icon) + Woodcut Line (loading);
Heraldic Shield devices; Acanthus & Bezant border; table chrome (§6.3); the three
starred screen layouts; all five motion verbs.

**Deferred flourishes** — the Rota Fortunæ deluxe face; the Alchemical Orrery
menu mark; the Knight-vs-Snail joust; the Self-Growing Vine reveal; the Inhabited
Initial's animated counters; carpet-page treatments. Each is described in §6 and
composes from the §8 verbs; draft them when wanted.

## Forward hooks (boom2)

The v2 core rework lands on fertile ground — **herbals were literally illuminated
manuscripts of plant recipes**, so the boom2 fiction needs no translation:

- **The 12 Brewers** → historiated miniatures: portrait roundels in the gilt-frame
  treatment of §6.1, marginalia poses for picks/emotes.
- **The Pantry** (Sage, Mint, Nightshade, Saffron…) → herbal marginalia; each
  bucket gets a Scribe-register plant study with a rubricated name.
- **The Grimoire & spells** → rubricated entries; the reserve pick gets the
  Illuminator treatment (a gilt initial among plain entries).
- **The Vote** → the wax seal becomes a ballot: pressing your seal *is* the vote.

None of this is designed yet — these are seams, recorded so the v2 work knows the
system extends to meet it.
