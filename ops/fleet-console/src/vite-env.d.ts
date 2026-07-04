/// <reference types="vite/client" />

// noVNC ships no type declarations; its package `exports` maps the bare
// specifier to core/rfb.js (default export = the RFB class). We only touch it
// through the small RfbLike surface in useRfb, so an untyped module is fine.
declare module "@novnc/novnc";
