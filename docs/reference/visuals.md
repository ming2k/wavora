# Visual Reference

This page lists the built-in compositions, tuning values, and shared visual
behavior. For the design principles behind the stage, use
[Visual Design](../explanation/visual-design.md).

## Stage Properties

| Property | Current value or behavior |
|----------|---------------------------|
| Base background | Near-black `#030508` |
| Layout | Main visual stage with a 304 px tabbed inspector when at least 480 px remains for the stage; stacked otherwise, with a compact preview density below 340 px stage height |
| Content-page background | Full active composition in window coordinates |
| Visual-page shell | Low-detail ambient field outside the clipped stage |
| Coordinate system | Full-window logical pixels on content pages; local logical pixels in the stage |
| View continuity | The same renderer, palette, animation phase, tuning, and audio state persist across views |
| Glow sizing | Width-relative for common layouts, height-capped on ultrawide windows |
| Atmosphere | Independent layer with up to four composable light sources |
| Atmosphere coordinates | Normalized window coordinates; values outside `0..=1` place centres beyond the window |
| Preset switching | Preserves playback state, information architecture, and control positions |
| Feature input | Spectrum, pitch, loudness, low/mid/high bands, and transients from each PCM buffer |
| Temporal response | Time-based attack and release smoothing |
| Paused response | Envelopes decay instead of freezing the final feature frame |
| Localization | Preset names, descriptions, controls, and metrics come from `wavora-i18n` |

The inspector keeps its heading and Composition/Atmosphere tabs outside the
scroll viewport. The two views opt into Optics' compact indicator tab variant:
hover adds a quiet themed surface, while selection is shown by an accent bar
whose independently sprung edges stretch and settle during a switch. This
avoids forcing the tab and panel backgrounds to match. The two hit targets
split the available rail evenly; the indicator is centred to the label width
and occupies a separate track below the clickable content. Each tab owns a
separate scroll position. Composition uses a single
preset selector plus a response card; Atmosphere separates scene
layers, material, and light-source editing into distinct surfaces. Boolean
layers use checkboxes, source mutations use buttons, and exclusive choices use
dropdowns so visual style follows interaction semantics. The inspector and
dropdown overlays are opaque contrast surfaces: stage content cannot show
through controls or an open option list. Re-clicking a dropdown trigger closes
the list once, and scrolling the inspector closes it before its anchor moves;
while open, the list is positioned within the inspector's scroll viewport. At
short window heights, the preview
keeps only its identity, playback state, and compact audio summary; descriptive
response copy and metric cards return automatically when enough height exists.

## Built-in Compositions

| Index | Name | Primary visual model |
|-------|------|----------------------|
| `0` | Particle Veil | Perspective particle curtain, depth dust, and three-band streamlines |
| `1` | Pulse Tunnel | Depth tunnel |
| `2` | Orbital Core | Frequency orbits |
| `3` | Spectral Void | Eclipse |
| `4` | Vinyl Halo | Vinyl grooves |
| `5` | Star River | Three-band flow field |

## Visual Tuning

| Configuration key | Default | Accepted range |
|-------------------|---------|----------------|
| `visual_intensity` | `1.0` | `0.45`–`1.75` |
| `visual_motion` | `1.0` | `0.35`–`1.65` |
| `visual_depth` | `1.0` | `0.50`–`1.50` |
| `visual_glow` | `0.9` | `0.25`–`1.50` |

The application normalizes values to these ranges and persists them
atomically with `visual_preset`. The preset index is normalized against the
six built-in presets.

## Atmosphere

Atmosphere configuration is persisted independently from `visual_preset`.
Disabling `composition_visible` keeps the base and atmosphere layers while
omitting the selected composition's foreground geometry.

Each source contains:

| Property | Accepted value or behavior |
|----------|----------------------------|
| `x`, `y` | Normalized position, safely bounded to `-4.0..=5.0`; the UI exposes `-2.0..=3.0` |
| `radius` | `0.05..=2.0` effective window widths |
| `shape` | `circle`, `oval`, or `beam`; independent from falloff and material |
| `aspect` | `1.0..=4.0` long-axis multiplier for oval and beam sources |
| `rotation` | `-0.5..=0.5` turns for oval and beam direction |
| `intensity` | `0.0..=2.0`, multiplied by the shared glow tuning |
| `palette` | Follow the active composition or use a custom HSL-derived ramp |
| `falloff` | `diffuse`, `focused`, or `halo` |
| `drift` | Independent circular drift amplitude, `0.0..=0.18` |
| `phase` | Independent normalized animation phase |
| `audio_response` | `none`, `energy`, `bass`, `mid`, `treble`, or `onset` |
| `audio_scale` | `0.0..=0.8` additional radius at full response |
| `audio_intensity` | `0.0..=1.5` additional brightness at full response |

An optional material field renders between the base colour and immediate light
sources:

| Property | Accepted value or behavior |
|----------|----------------------------|
| `kind` | `none`, `watercolor`, or `caustics` |
| `intensity` | `0.0..=1.5` |
| `scale` | `0.45..=2.2` procedural feature scale |
| `motion` | `0.0..=1.0` overscan drift amount |
| `palette` | Follow the active composition or use a custom HSL-derived ramp |
| `seed` | Stable deterministic field variation |

Only an individual source lobe's intersecting radius is submitted to Flux, so
a source beyond the window has no fill cost when its light cannot reach the
visible scene. Circle sources use one lobe. Oval and beam sources use a fixed,
bounded nine-lobe approximation so anisotropic light can respond to audio
without per-frame texture generation or upload.
The render snapshot shares atmosphere data across viewport clones and replaces
it only when the user changes a setting. Material textures use a bounded
512-pixel long edge and a 224-pixel minimum short edge. They are cached by
field configuration, relevant source geometry, palette, and target aspect
ratio; animation moves the overscanned texture instead of regenerating it.
Audio response affects the immediate light layer, while a source-guided
watercolour texture uses the source's stable base geometry. This keeps audio
animation on the render thread and avoids regenerating a CPU texture per frame.

## Application Background Presentation

Outside the visual page, atmosphere and the selected preset's composition
render in full-window coordinates behind the application surfaces. Opening
the visual page moves the renderer into the clipped stage viewport. The shell
outside that viewport uses the same atmosphere configuration and diffuse
falloff, so navigation does not switch to a separately maintained glow.

Content panels, navigation, and playback controls own their contrast through
dark translucent surfaces. The home surface stays close to the visual stage's
effective translucency; denser library, playlist, lyric, and settings surfaces
use a slightly stronger scrim. This keeps composition identity in one renderer
and keeps readability policy in the UI layer instead of maintaining a second
set of background geometry.

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
