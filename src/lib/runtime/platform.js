/**
 * Desktop Linux (WebKitGTK). Android webviews also report "Linux", but the mobile
 * shells never open desktop windows.
 */
export function isLinuxDesktop() {
  if (typeof navigator === "undefined") return false;
  const userAgent = String(navigator.userAgent || "");
  return /linux/i.test(userAgent) && !/android/i.test(userAgent);
}
