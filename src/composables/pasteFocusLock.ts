/**
 * Sync lock so App.vue won't steal focus mid-paste (event bus is too late).
 * Reference-counted: overlapping pastes (rapid Enter presses) must not let
 * the first one finishing unlock while another is still in flight.
 */
let lockCount = 0;

export function setPasteFocusLock(value: boolean) {
  if (value) {
    lockCount += 1;
  } else if (lockCount > 0) {
    lockCount -= 1;
  }
}

export function isPasteFocusLock(): boolean {
  return lockCount > 0;
}
