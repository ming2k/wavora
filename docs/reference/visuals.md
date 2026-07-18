# Visual Reference

This page lists the built-in subject effects, ambient modules, tuning values,
and shared visual behavior. For the design principles behind the stage, use
[Visual Design](../explanation/visual-design.md).

## Stage Properties

| Property | Current value or behavior |
|----------|---------------------------|
| Base background | Near-black `#030508` |
| Layout | Main visual stage with a 304 px tabbed inspector when at least 480 px remains for the stage; stacked otherwise, with a compact preview density below 340 px stage height |
| Content-page background | Enabled subject and ambient layers in window coordinates |
| Visual-page shell | Low-detail ambient field outside the clipped stage |
| Coordinate system | Full-window logical pixels on content pages; local logical pixels in the stage |
| View continuity | The same renderer, palette, animation phase, tuning, and audio state persist across views |
| Subject | Independent primary effect layer; may be disabled without changing ambient light |
| Ambient | Independent procedural material and up to four composable sources; may be disabled without changing the subject |
| Ambient coordinates | Normalized window coordinates; values outside `0..=1` place centers beyond the window |
| Effect switching | Preserves playback state, ambient configuration, information architecture, and control positions |
| Feature input | Spectrum, pitch, loudness, low/mid/high bands, and transients from each PCM buffer |
| Temporal response | Time-based attack and release smoothing |
| Paused response | Envelopes decay instead of freezing the final feature frame |
| Localization | Effect names, descriptions, controls, and metrics come from `wavora-i18n` |

The inspector keeps its heading and Subject/Ambient tabs outside the scroll
viewport. Each tab owns a separate scroll position. Subject has its own enable
switch, one effect selector, and a response card. Ambient has a separate enable
switch and divides material and source editing into distinct surfaces. Both
switches preserve the disabled module's settings.

Boolean layers use checkboxes, source mutations use buttons, and exclusive
choices use dropdowns. Inspector and dropdown surfaces remain opaque so stage
content cannot reduce control contrast. At short window heights, the preview
keeps only its identity, playback state, and compact audio summary; descriptive
copy and metric cards return when enough height is available.

## Built-in Subject Effects

| Index | Name | Primary visual model |
|-------|------|----------------------|
| `0` | Particle Veil | Perspective particle curtain, depth dust, and three-band streamlines |
| `1` | Pulse Tunnel | Depth tunnel |
| `2` | Orbital Core | Frequency orbits |
| `3` | Spectral Void | Eclipse |
| `4` | Vinyl Halo | Vinyl grooves |
| `5` | Star River | Three-band flow field |
| `6` | Prism Ribbons | Layered sampled-spectrum ribbons with depth particles |
| `7` | Luminous Bloom | Radial spectrum petals driven by pitch and transients |
| `8` | Ripple Field | Canvas-native point surface adapted from the Optics ripple model |
| `9` | Particle Terrain | Projected traveling-ridge heightfield adapted from the Optics terrain model |
| `10` | Cover Relief | Album artwork UV tiles projected across audio-reactive cuboid columns |

The subject registry stores each effect's palette and render function together.
Adding an effect therefore requires one registry entry and one renderer, not a
second stage-level dispatch table.

## Subject Tuning

| Configuration path | Default | Accepted range |
|--------------------|---------|----------------|
| `visual_stage.subject.enabled` | `true` | Boolean |
| `visual_stage.subject.effect` | `0` | `0`–`10` (normalized to the effect registry) |
| `visual_stage.subject.tuning.intensity` | `1.0` | `0.45`–`1.75` |
| `visual_stage.subject.tuning.motion` | `1.0` | `0.35`–`1.65` |
| `visual_stage.subject.tuning.depth` | `1.0` | `0.50`–`1.50` |
| `visual_stage.subject.tuning.glow` | `0.9` | `0.25`–`1.50` |

The application normalizes values to these ranges and persists the subject
module atomically with the ambient module.

## Ambient Layer

Ambient light is persisted as `visual_stage.ambient`, independently from
`visual_stage.subject`. Disabling either module does not mutate or reset the
other module's settings.

Each source contains:

| Property | Accepted value or behavior |
|----------|----------------------------|
| `x`, `y` | Normalized position, safely bounded to `-4.0..=5.0`; the UI exposes `-2.0..=3.0` |
| `radius` | `0.05..=2.0` effective window widths |
| `shape` | `circle`, `oval`, or `beam`; independent from falloff and material |
| `aspect` | `1.0..=4.0` long-axis multiplier for oval and beam sources |
| `rotation` | `-0.5..=0.5` turns for oval and beam direction |
| `intensity` | `0.0..=2.0`, multiplied by the shared glow tuning |
| `palette` | Follow the active subject effect or use a custom HSL-derived ramp |
| `falloff` | `diffuse`, `focused`, or `halo` |
| `drift` | Independent circular drift amplitude, `0.0..=0.18` |
| `phase` | Independent normalized animation phase |
| `audio_response` | `none`, `energy`, `bass`, `mid`, `treble`, or `onset` |
| `audio_scale` | `0.0..=0.8` additional radius at full response |
| `audio_intensity` | `0.0..=1.5` additional brightness at full response |

An optional material renders between the base color and immediate sources:

| Property | Accepted value or behavior |
|----------|----------------------------|
| `kind` | `none`, `watercolor`, `caustics`, `aurora`, or `nebula` |
| `intensity` | `0.0..=1.5` |
| `scale` | `0.45..=2.2` procedural feature scale |
| `motion` | `0.0..=1.0` overscan drift amount |
| `palette` | Follow the active subject effect or use a custom HSL-derived ramp |
| `seed` | Stable deterministic material variation |

Only an individual source lobe's intersecting radius is submitted to Flux, so
a source beyond the window has no fill cost when its light cannot reach the
visible scene. Circle sources use one lobe. Oval and beam sources use a fixed,
bounded nine-lobe approximation so anisotropic light can respond to audio
without per-frame texture generation or upload.

The render snapshot shares ambient data across viewport clones and replaces it
only when a setting changes. Material textures use a bounded 512-pixel long
edge and a 224-pixel minimum short edge. They are cached by material settings,
relevant source geometry, palette, and target aspect ratio; animation moves an
overscanned texture instead of regenerating it.

## Cover Relief Projection

Cover Relief divides the current square artwork into a 10×10 UV grid. Each UV
tile is mapped through an affine transform onto the top face of one projected
cuboid. With zero response, the 100 faces are coplanar and reconstruct the
complete cover. Spectrum bands, bass, and onset lift individual cuboids while
the same UV pieces remain attached, producing a fragmented album-art relief
rather than 100 duplicated covers. Two shaded side faces provide thickness.
Tracks without artwork use the active subject palette as a deterministic
fallback.

The artwork cache retains the Flux image for the full paint call. The visual
snapshot carries only a temporary non-owning handle, so artwork ownership and
decoding remain in the media/UI boundary rather than the reusable renderer.

## Configuration Migration

Configuration version 9 stores both modules under `visual_stage`. Older
top-level `visual_preset`, `visual_intensity`, `visual_motion`, `visual_depth`,
`visual_glow`, and `atmosphere` values are migrated during normalization.
The former `visual_stage.lighting` key is accepted as an alias for
`visual_stage.ambient`.
Legacy `composition_visible` becomes `visual_stage.subject.enabled`, preserving
an atmosphere-only setup as an ambient-only setup.

## Application Background Presentation

Outside the visual page, enabled subject and ambient modules render in full-window
coordinates behind application surfaces. Opening the visual page moves the
same renderer into the clipped stage viewport. The shell outside that viewport
uses the same ambient configuration, so navigation does not switch to a separate
glow implementation.

Content panels, navigation, and playback controls own their contrast through
dark translucent surfaces. This keeps visual identity in one renderer and
readability policy in the UI layer.

## Particle Veil Mapping

| Input | Visual response |
|-------|-----------------|
| Pitch | Folding speed |
| Midrange energy | Surface undulation |
| Spectral centroid | Streamline steering |
| Bass energy | Curvature and particle size |
| Transients | Curvature and particle size |
| Low/mid/high bands | Three-band streamlines |

Particle Veil uses perspective projection. Its stage coordinates remain in
logical pixels and do not depend on the window device scale, including on a
2× HiDPI display.
