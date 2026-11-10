/* The village page. Each module registers what it draws, what its objects
 * are and its labels (registry.js); this only loads them, in
 * order, and starts the page. */
import './themes/medieval.js';
import './themes/modern.js';
import './themes/future.js';
import './layout.js';
import './lifecycle.js';
import './ship.js';
import './gatehouse.js';
import './weather.js';
import './bounces.js';
import './features.js';
import './envrow.js';
import './voyages.js';
import './worlds.js';
import './panels.js';
import './dialogs.js';
import './labels.js';
import './key.js';
import './screensaver.js';
import { start } from './data.js';

start();
