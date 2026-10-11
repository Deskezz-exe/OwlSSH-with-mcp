// Questions the connect flow asks the user: a password, a key passphrase,
// or whether to trust a host key. One question is shown at a time.

export type PromptRequest =
  | { kind: "password"; target: string }
  | { kind: "passphrase"; keyPath: string }
  | {
      kind: "hostKey";
      changed: boolean;
      target: string;
      fingerprint: string;
      algorithm: string;
    };

export type PromptAnswer =
  | { kind: "password"; password: string; save: boolean }
  | { kind: "passphrase"; passphrase: string }
  | { kind: "hostKey"; trust: true };

class PromptState {
  current = $state<PromptRequest | null>(null);
  private resolve: ((answer: PromptAnswer | null) => void) | null = null;

  /** Shows the question and waits. Resolves to null when the user cancels. */
  ask(request: PromptRequest): Promise<PromptAnswer | null> {
    this.resolve?.(null);
    this.current = request;
    return new Promise((resolve) => (this.resolve = resolve));
  }

  answer(answer: PromptAnswer | null) {
    const resolve = this.resolve;
    this.resolve = null;
    this.current = null;
    resolve?.(answer);
  }
}

export const prompt = new PromptState();
