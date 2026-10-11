/**
 * The inline script the root layout runs in <head> before the body paints (rendered with the request's CSP nonce). The
 * store is light only, as Silicon Writings is: the script pins <html data-theme="light"> so Silicon UI's foundation
 * never follows a dark system setting, and marks <html data-js> so controls that only work with script (copy buttons)
 * stay hidden in a browser without it. The embed surface keeps no theme of its own (its colour scheme comes only from
 * its `theme` parameter, or the iframe canvas turns opaque).
 */
export const BOOT_SCRIPT = `(function(){var r=document.documentElement;r.setAttribute("data-js","");if(r.getAttribute("data-surface")==="embed")return;r.setAttribute("data-theme","light");r.style.colorScheme="light"})();`;
