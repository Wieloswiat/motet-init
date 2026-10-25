// The export loaded the engine and demo as classic scripts, in order. The
// engine mentions `module`, so the bundler wraps it as CommonJS and runs it
// lazily; a static import of the demo could then run first and find no
// window.Motet. Awaiting the demo after the engine keeps the export's order.
import '@motet/design/engine';

await import('@motet/design/demo');
