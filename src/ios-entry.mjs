import { installIOSBridge } from './ios-bridge.mjs';
installIOSBridge();
await import('./main.mjs');
