# wavora-ui

`wavora-ui` is Wavora's product-level composition layer over Optics Iris.

It owns:

- Wavora design tokens and theme recipes;
- reusable combinations of Optics widgets;
- presentation-only component props and interaction results.

It does not own:

- generic widget layout, input, accessibility, or animation mechanics
  (Optics Lens/Iris);
- application state, commands, localization lookup, or persistence (`wavora`);
- audio-reactive drawing (`wavora-visuals`).

Components must not accept `wavora::app::App`. Callers resolve domain data and
localized strings before building a component, then translate its returned
interaction into an application action. This keeps the dependency direction
one-way and makes components usable in headless tests.
