# Wavora Design Direction

The visual direction takes inspiration from Mineradio's immersive stage,
lyric hierarchy, and warm golden-glass aesthetic without copying its Electron
page structure. The native Optics implementation follows these rules:

- The background starts from near-black `#030508`; accent colors are reserved
  for spatial lighting and interaction feedback.
- Large panels use restrained transparency. Edge highlights, dark material
  depth, and inner and outer shadows create the glass effect instead of broad
  white gradients.
- The bottom playback console is a stable visual anchor. Changing visual
  presets must not disrupt play, pause, or track navigation.
- Particles remain orderly with subtle breathing motion. Frequency, pitch,
  loudness, and transients map to recognizable spatial properties rather than
  arbitrary visual noise.
- The visual page uses a main stage with a control rail. Compositions stay on
  a continuous canvas, while presets and parameters sit in a dense sidebar.
  Users can adjust and persist response intensity, motion speed, spatial
  depth, and glow.
- All visual names, response descriptions, and live metrics come from the
  i18n catalog. Rendering modules contain no user-facing English or Chinese
  copy, which prevents mixed-language output after switching languages.
- Normal animation quality is maintained while the window is visible.
  Performance optimizations use caching and avoid redundant computation.
- Lyrics and 3D media displays are future stage layers. They must not obscure
  core playback controls or readable information.

The six built-in compositions are Particle Veil, Pulse Tunnel, Orbital Core,
Spectral Void, Vinyl Halo, and Star River. They use a particle curtain, depth
tunnel, frequency orbits, eclipse, vinyl grooves, and a three-band flow field,
respectively. Switching presets does not change the information architecture
or control positions. Audio feature frames are generated with each PCM buffer.
The visual layer applies time-based attack and release smoothing consistently,
allowing the composition to decay naturally during pauses instead of freezing
on the last frame.

Particle Veil uses a perspective-projected particle mesh, depth dust, and
three-band streamlines. Pitch controls folding speed, midrange controls surface
undulation, spectral centroid changes steering, and bass and transients affect
curvature and particle size. Stage coordinates remain in logical pixels and do
not depend on the window's device scale, preventing the composition from being
cropped into a narrow band at 2× HiDPI.
