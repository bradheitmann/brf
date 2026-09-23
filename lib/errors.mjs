// A failure the command line reports as a plain message instead of a stack trace.
export class BrfError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "BrfError";
    this.code = code;
  }
}
