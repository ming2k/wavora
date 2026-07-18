# Visual Design

Wavora presents playback as an immersive stage rather than a collection of
independent dashboard panels. Its direction draws on Mineradio's stage, lyric
hierarchy, and warm golden-glass aesthetic without reproducing Mineradio's
Electron page structure. The visual system uses depth, restrained materials,
and audio-responsive motion to reinforce the music while keeping playback
controls predictable.

For the built-in subject effects, tuning ranges, and exact rendering behavior,
use the [Visual Reference](../reference/visuals.md).

## Stable Stage

The playback console is the stable visual anchor. Changing a subject effect
alters the visual character but not the location or behavior of play, pause, track
navigation, and visual controls. This continuity lets the stage feel dynamic
without making routine playback harder to operate.

The Visual Stage follows an editor layout rather than a long settings form. A
minimum-width preview remains the primary surface, while a tabbed inspector
separates subject decisions from ambient construction. Inspector cards
group settings by responsibility, and the tab bar stays visible while its
content scrolls. When the window cannot preserve a useful preview width, the
inspector stacks below the preview instead of compressing both columns. The
stacked preview uses a compact information density rather than allowing desktop
cards to overflow a shorter stage. Inspector and popup surfaces remain opaque;
the ambient field is visible around the editing surfaces, never through their
content.

The enabled subject and ambient modules render behind every content page. The
visual page moves the same renderer into a
bounded viewport beside a dense control rail; it does not replace the scene
with a separate ambient interpretation. Palette, animation phase, tuning,
audio state, transition, and geometry therefore remain continuous across
navigation. Stage geometry uses local coordinates and cannot draw outside its
viewport. Content pages preserve readability through UI-owned translucent
surfaces rather than a second, independently maintained background renderer.

## Subject and Ambient Modules

Subject is the attention-carrying effect; Ambient light is the environment beneath
it. The modules have independent enable state. Changing or disabling the
subject does not reset light placement, motion, falloff, or custom color. A
source can follow the subject palette when a coordinated result is preferred.
Disabling the subject leaves an ambient-only listening space, while disabling
ambient light preserves a clean subject on the dark base.

Sources use normalized window coordinates. The visible window occupies
`0..=1` on each axis, but source positions may extend beyond that range. This
allows a light centre to remain outside the window while only its diffuse tail
enters the scene. Each source owns its own position, radius, intensity,
falloff, colour policy, and drift phase.

Source geometry and source response are also independent. Circle, oval area,
and directional beam shapes can all use diffuse, focused, or halo falloff. A
source may remain static or bind its scale and brightness to broad energy,
bass, midrange, treble, or transients; this binding does not alter the selected
subject or procedural material. Elongated sources keep the same normalized
off-window coordinate system as circles.

The source count is deliberately bounded. This keeps the editor legible and
caps full-screen gradient overdraw on integrated GPUs. Watercolor diffusion,
water caustics, aurora curtains, and nebula clouds are separate material fields
rather than light shapes. Watercolor uses source geometry to place granulated
pigment plumes; caustics uses a warped interference field. All materials render
beneath the immediate sources and can follow the subject palette or use a
custom ramp.

Subject effects use a registry that colocates palette and renderer. Shared
projection helpers let Ripple Field and Particle Terrain reuse point-grid,
depth, and submission rules while keeping distinct height functions. Cover
Relief uses the current album artwork as one UV surface split across a grid of
cuboid tops: the pieces reconstruct the full cover when coplanar, then remain
attached as spectrum and transients lift individual columns.

Direct manipulation belongs in a future stage-canvas interaction primitive.
The current Lens widget surface does not provide a transparent custom drag
region with current-frame geometry, so source positions use explicit normalized
controls rather than a parallel input path with ambiguous hit testing.

Material fields are deterministic CPU-generated textures uploaded only after
their relevant settings or target aspect ratio changes. A bounded overscan
area lets the texture drift without exposing an edge, avoiding per-frame CPU
generation and GPU upload. Full-window and Visual Stage variants are cached
separately so neither is stretched from the other's aspect ratio.

## Material and Contrast

The palette begins near black and reserves brighter accents for spatial light
and interaction feedback. Large surfaces use restrained transparency, dark
material depth, edge highlights, and shadows. This produces a glass-like
surface without reducing text contrast through broad white gradients.

## Legible Audio Response

Audio features map to recognizable spatial properties. Frequency regions,
pitch, loudness, and transients influence motion, shape, and depth in ways that
remain consistent within a subject effect. Ordered particles and subtle
breathing motion preserve structure instead of turning every detected change
into visual noise.

The visual layer smooths feature changes over time. Attack preserves the
impact of new energy, while release lets a subject decay naturally during
a pause instead of freezing on its last frame.

## Localization and Performance

Visual names, response descriptions, controls, and live metrics come from the
localization catalog. Rendering data contains no user-facing language, which
keeps the entire stage consistent when the language changes.

Normal animation quality remains active while the window is visible.
Performance work focuses on caching and eliminating redundant computation so
that optimization does not silently change the visual character.

## Future Layers

Lyrics and three-dimensional media displays can extend the stage as additional
layers. Any such layer must preserve readable information and keep core
playback controls unobstructed.
