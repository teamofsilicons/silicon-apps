/**
 * Each app's own picture, by app id (Anil, 2026-11-14): the Core Apps cards on home and the cover at the top of an app's
 * page use it. `focus` is where to keep in frame when the picture is cropped (CSS background or object position).
 */
export type AppPicture = { src: string; focus?: string };

/** Each core app's own picture, from Cosmos (cosmos.so, 2026-11-14; other people's uploads, so check the rights
 * before shipping): a frosted glass folder (mbatten), a pink envelope (sunset mood), and for Ring a man taking a call
 * on a brick of a mobile phone (cariokaboy, cosmos.so/e/70297960; Anil's pick; a real, recognisable person, so it needs
 * a model release as well as the photo's rights before shipping). */
export const APP_PICTURES: Record<string, AppPicture> = {
  briefcase: { src: "/home/briefcase.webp" },
  dm: { src: "/home/dm.webp" },
  ring: { src: "/home/ring.webp", focus: "55% center" },
  // Ready for when these join the core apps (Anil, 2026-11-14), from Cosmos, the same rights caveat: keys and a brass key
  // tag in a dish (Zita Fernandez; the tag carries a maker's name), a white locker-style vending wall (uploader not
  // named; its screen carries a fashion brand's name), and a grey Braun slide projector (uploader not named).
  "silicon-accounts": { src: "/home/silicon-accounts.webp", focus: "center 40%" },
  "silicon-apps": { src: "/home/silicon-apps.webp" },
  present: { src: "/home/present.webp" },
};

