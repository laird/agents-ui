/* The village page. Each module registers what it draws, what its objects
 * are and its labels (registry.js); this only loads them and starts the
 * page. The themes come first, in the order the Style picker lists them,
 * then layout.js. modules.js, which build.rs writes (#520), then imports
 * every other module, sorted by path, so adding a module edits no shared
 * file. Each module runs after its own imports, and where modules register
 * into one list they say where in it (`order`), so nothing else depends on
 * the order they load in. */
import './themes/medieval.js';
import './themes/modern.js';
import './themes/future.js';
import './layout.js';
import './modules.js';
import { start } from './data.js';

start();
