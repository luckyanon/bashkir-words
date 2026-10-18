import { Capacitor, registerPlugin } from '@capacitor/core';
import { App } from '@capacitor/app';
import { Preferences } from '@capacitor/preferences';
import { Haptics } from '@capacitor/haptics';
window.Capacitor = Capacitor;
const BackgroundMotion=registerPlugin('BackgroundMotion');
const DailyGift=registerPlugin('DailyGift');
window.BashkirNativePlugins = { App, Preferences, Haptics, BackgroundMotion, DailyGift };
await import('./main.mjs');

