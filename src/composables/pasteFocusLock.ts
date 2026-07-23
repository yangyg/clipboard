/** Sync lock so App.vue won't steal focus mid-paste (event bus is too late). */
let locked = false;

export function setPasteFocusLock(value: boolean) {
  locked = value;
}

export function isPasteFocusLock(): boolean {
  return locked;
}
