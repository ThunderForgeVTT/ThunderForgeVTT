/** A new seed for the dice: eight hex digits a person can read back and
 * type in again. The seed, not this function, is what makes a roll
 * reproducible. */
export function freshSeed(): string {
  const bytes = new Uint8Array(4);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join(
    "",
  );
}
