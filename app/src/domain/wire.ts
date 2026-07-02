// Shared reply decoding for the module clients.
//
// Every `*Reply` enum serializes as a single-variant object
// (`{"Messages": [...]}`) or, for unit variants, a bare string. replyVariant
// unwraps the expected variant or throws — a mismatch means the module and
// this client disagree about the interface, which must surface loudly.

export const replyVariant = <T>(reply: unknown, variant: string): T => {
  if (
    typeof reply === "object" &&
    reply !== null &&
    variant in (reply as Record<string, unknown>)
  ) {
    return (reply as Record<string, T>)[variant];
  }
  throw new Error(`unexpected module reply: wanted ${variant}`);
};
