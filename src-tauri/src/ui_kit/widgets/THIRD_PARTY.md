# Third-party attribution

## longbridge/gpui-component

Apex Terminal's `ui_kit::widgets` library is inspired by Longbridge's
[gpui-component](https://github.com/longbridge/gpui-component) — visual
language, API surface, variant taxonomy, and naming.

- Source: https://github.com/longbridge/gpui-component
- Commit sampled: `42ae00839e24c10f55ea0fe88f547b5366a19404`
- License: Apache-2.0 / MIT (dual-licensed)

We are NOT porting code line-for-line. gpui-component targets GPUI
(retained-mode); this library targets egui (immediate-mode). The port is
API/visual spec only. Where individual code blocks are ported verbatim,
preserve copyright headers and add a `// Adapted from gpui-component:
<file>` comment.

## License compliance

- Apache-2.0 obligations satisfied via this NOTICE-style file.
- No code is shipped that requires source distribution.
